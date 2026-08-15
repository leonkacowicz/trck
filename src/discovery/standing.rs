//! Where the local tracker branch stands against the remote-tracking one, and what to do
//! about it.
//!
//! Split from [`super::source`] because it is a self-contained question about two revisions
//! — `source` decides *which tracker*, this decides *which of its two refs* — and because
//! `trck sync` will want the same answer for a different reason.

use super::source::TRACKER_REF;
use std::path::Path;

/// Bring the local branch into a state worth reading, or say why it is not.
///
/// Local is what gets read either way — it is the only one that can hold work nobody else
/// has. This decides whether it needs catching up first, and whether the reader needs
/// telling.
pub(super) fn reconcile(cwd: &Path, local: &str, remote: &str) -> Result<(), String> {
    match standing(cwd, local, remote)? {
        Standing::Same | Standing::Ahead => Ok(()),
        // A fast-forward and nothing else: the compare-and-swap names the value just read,
        // so a concurrent write between the read and the move is refused rather than
        // overwritten. This is the only ref move a *read* is allowed to make.
        Standing::Behind => crate::git::refs::update_ref(cwd, &format!("refs/heads/{TRACKER_REF}"), remote, Some(local)),
        Standing::Diverged => {
            // Local wins, because it holds work that exists nowhere else. Saying so is the
            // whole point: the alternative is a listing quietly missing whatever landed
            // remotely. On stderr, so piped output stays parseable.
            eprintln!(
                "warning: {TRACKER_REF} and origin/{TRACKER_REF} have diverged; reading {TRACKER_REF}, \
                 which is missing whatever landed remotely — run `trck sync` to reconcile"
            );
            Ok(())
        },
    }
}

/// Where the local branch stands against the remote-tracking one.
#[derive(Debug, PartialEq, Eq)]
enum Standing {
    Same,
    /// Local holds commits the remote does not — unpushed writes.
    Ahead,
    /// The remote has moved and local has nothing of its own.
    Behind,
    /// Both moved. Rare, and exactly what a failed push plus someone else's write leaves.
    Diverged,
}

fn standing(cwd: &Path, local: &str, remote: &str) -> Result<Standing, String> {
    if local == remote {
        return Ok(Standing::Same);
    }
    let local_reaches_remote = crate::git::is_ancestor(cwd, local, remote)?;
    let remote_reaches_local = crate::git::is_ancestor(cwd, remote, local)?;
    Ok(match (local_reaches_remote, remote_reaches_local) {
        (true, _) => Standing::Behind,
        (_, true) => Standing::Ahead,
        _ => Standing::Diverged,
    })
}

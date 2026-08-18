//! Where the local tracker branch stands against the remote-tracking one, and what to do
//! about it.
//!
//! Split from [`super::source`] because it is a self-contained question about two revisions
//! — `source` decides *which tracker*, this decides *which of its two refs* — and because
//! `trck sync` will want the same answer for a different reason.

use super::source::TRACKER_REF;
use std::path::Path;

/// How many local commits the remote has not got.
///
/// Zero when everything is shared, when there is no local branch, and when there is no
/// remote-tracking ref to be ahead of — the last because a tracker with no remote is not
/// *pending*, it is simply local, and telling someone to `trck sync` a repository with
/// nowhere to sync to is noise.
pub(crate) fn pending(cwd: &Path) -> Result<usize, String> {
    let (Some(_), Some(_)) = (crate::git::rev_parse(cwd, TRACKER_REF)?, crate::git::rev_parse(cwd, &tracking())?) else {
        return Ok(0);
    };
    let range = format!("{}..{TRACKER_REF}", tracking());
    crate::git::stdout(cwd, &["rev-list", "--count", &range])?.trim().parse().map_err(|e| format!("counting unpushed commits: {e}"))
}

/// The remote-tracking ref for the tracker branch.
pub(crate) fn tracking() -> String {
    format!("origin/{TRACKER_REF}")
}

/// What the local-versus-remote rule decided, before anybody says anything about it.
///
/// Split from the saying because the rule now has two audiences with opposite needs. A read
/// verb runs once and prints a warning; `serve` runs the same rule on a timer and has to log
/// a *change* — a poll loop printing the read verb's warning every thirty seconds would bury
/// the one line that matters under a page of "still diverged".
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Resolution {
    /// Nothing to do: local already holds everything the remote has, or more.
    Local,
    /// Local was behind and has been fast-forwarded onto the remote's tip.
    FastForwarded,
    /// Behind, and somebody has the branch checked out — so nothing moved and the
    /// remote-tracking ref is what to read. See [`resolve`] for why.
    ReadTracking,
    /// Both moved. Local wins, because it holds work that exists nowhere else, and the
    /// caller has to say so.
    Diverged,
}

impl Resolution {
    /// The ref this resolution says to read.
    pub(crate) fn rev(&self) -> String {
        match self {
            Resolution::ReadTracking => tracking(),
            _ => TRACKER_REF.to_string(),
        }
    }
}

/// Bring the local branch into a state worth reading, and say what that took.
///
/// Local is the answer almost always — it is the only one that can hold work nobody else
/// has. This decides whether it needs catching up first, and the one case where the
/// remote-tracking ref answers instead.
///
/// **It does not fetch.** Discovering that the remote moved is the caller's business: for a
/// read verb that never happens, and for `serve` it is a timer.
pub(crate) fn resolve(cwd: &Path, local: &str, remote: &str) -> Result<Resolution, String> {
    match standing(cwd, local, remote)? {
        Standing::Same | Standing::Ahead => Ok(Resolution::Local),
        // Behind, and somebody has the branch checked out. The fast-forward below would move
        // the branch under their worktree, leaving it reporting the newer commits inverted —
        // a desync arriving from `trck list`, which has no business detaching anyone to avoid
        // it. So nothing moves and the remote-tracking ref answers: it holds exactly the
        // commits the fast-forward would have brought.
        Standing::Behind if crate::git::worktree::is_checked_out(cwd, &format!("refs/heads/{TRACKER_REF}"))? => Ok(Resolution::ReadTracking),
        // A fast-forward and nothing else: the compare-and-swap names the value just read,
        // so a concurrent write between the read and the move is refused rather than
        // overwritten. This is the only ref move a *read* is allowed to make.
        Standing::Behind => {
            crate::git::refs::update_ref(cwd, &format!("refs/heads/{TRACKER_REF}"), remote, Some(local))?;
            Ok(Resolution::FastForwarded)
        },
        Standing::Diverged => Ok(Resolution::Diverged),
    }
}

/// Apply the rule for a one-shot read: resolve, and print the one case the reader has to
/// know about.
pub(super) fn reconcile(cwd: &Path, local: &str, remote: &str) -> Result<String, String> {
    let resolution = resolve(cwd, local, remote)?;
    if resolution == Resolution::Diverged {
        // Local wins, because it holds work that exists nowhere else. Saying so is the
        // whole point: the alternative is a listing quietly missing whatever landed
        // remotely. On stderr, so piped output stays parseable.
        eprintln!("warning: {}", divergence());
    }
    Ok(resolution.rev())
}

/// The sentence for a diverged pair, worded once now that two callers say it.
pub(crate) fn divergence() -> String {
    format!(
        "{TRACKER_REF} and origin/{TRACKER_REF} have diverged; reading {TRACKER_REF}, \
         which is missing whatever landed remotely — run `trck sync` to reconcile"
    )
}

/// Re-read both refs and apply the rule to whatever they hold now.
///
/// `None` when there is nothing to compare: one of the two refs does not exist, which is a
/// tracker that has never been pushed or a clone whose branch has never been written.
/// Neither is a state the rule has anything to say about, and neither is an error.
///
/// For `serve`, which asks on a timer. Every other caller resolves its tracker once and
/// already holds both revisions by the time the rule is wanted.
pub(crate) fn reassess(cwd: &Path) -> Result<Option<Resolution>, String> {
    let (Some(local), Some(remote)) = (crate::git::rev_parse(cwd, TRACKER_REF)?, crate::git::rev_parse(cwd, &tracking())?) else {
        return Ok(None);
    };
    resolve(cwd, &local, &remote).map(Some)
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

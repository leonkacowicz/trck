//! Where a [`Changeset`] lands.
//!
//! Everything upstream of here — deriving the rollup, rendering the index, working out which
//! body file moved — happens on values. A backend is the only thing that makes any of it
//! real, which is what lets a tracker be a directory or a git ref without the verbs knowing
//! which.
//!
//! [`dir`] writes files. [`git`] builds a commit and moves a ref, touching neither the
//! working tree nor the caller's index — so a tracker write works from a dirty checkout on an
//! unrelated branch, which is the whole reason the tracker moves off one.
//!
//! Both take the [`Op`] alongside the changeset. A directory has nowhere to record what the
//! verb was asked to do and drops it; a commit carries it in the message, which is what makes
//! the history replayable rather than merely diffable.

use super::changeset::{Changeset, Edit};
use super::op::Op;

mod dir;
mod git;
mod sync;
// Writing a message is [`git`]'s; reading one back is nobody's yet. The replay path
// (`#5w9d7sq`) is `op_of`'s first consumer, and re-exporting it from here is that change's to
// make — which is what the crate-level `dead_code` expectation covers in the meantime.
mod message;

pub(crate) use dir::DirBackend;
pub(crate) use git::RefBackend;
pub(crate) use sync::sync;

/// The local branch a revision writes to.
///
/// Writes always land on `refs/heads/`, whatever the tracker was *read* from: a
/// remote-tracking ref is a copy of someone else's branch and moving it locally would make
/// this clone disagree with the remote it is named after. Stripping `origin/` is what turns a
/// fresh clone's only ref into the branch this write should create.
pub(crate) fn local_ref(rev: &str) -> String {
    format!("refs/heads/{}", local_branch(rev))
}

/// The same branch, spelled the way a revision is read and printed.
///
/// [`local_ref`] answers in the form `update-ref` demands. Anything that *reads* the branch
/// back or *shows* it to someone wants the short name — which is also what a clone that
/// already has the branch resolves to, so the first write and the second one name a body the
/// same way rather than the first one shouting `refs/heads/`.
pub(crate) fn local_branch(rev: &str) -> &str {
    rev.strip_prefix("refs/heads/").or_else(|| rev.strip_prefix("origin/")).unwrap_or(rev)
}

/// Get every worktree off `refname` so the ref can move, or say why one of them cannot be.
///
/// Called before the compare-and-swap rather than after, because git will not refuse the move
/// on anybody's behalf and so there is nothing to learn from letting it happen first. The cost
/// of that order is that a *lost* swap leaves a worktree detached for nothing — harmless, one
/// `git checkout -` from undone, and named in the note either way.
///
/// A worktree mid-operation is the one case that stops a write: see
/// [`worktree::in_progress`](crate::git::worktree). A locked one, or one that will not take the
/// update, is left where it is and named — a tracker write is not blocked by the state of
/// somebody else's disk, but the desync it leaves must not be silent.
///
/// The notes go to stderr, beside the divergence warning in [`crate::discovery::standing`] and
/// for the same reason: they are about the repository rather than about the tracker, and piped
/// output stays parseable.
pub(crate) fn release(cwd: &std::path::Path, refname: &str) -> Result<(), String> {
    use crate::git::worktree::State::{Busy, Free, Locked};
    let branch = local_branch(refname);
    for held in crate::git::worktree::holders(cwd, refname)? {
        match held.state {
            Busy(what) => return Err(refusal(&held, branch, what)),
            Locked => eprintln!("{}", left_on_it(&held, branch, "is locked")),
            Free => match crate::git::worktree::detach(&held.path, &held.head) {
                Ok(()) => eprintln!("{}", detached(&held, branch)),
                Err(why) => eprintln!("{}", left_on_it(&held, branch, &format!("could not be detached ({why})"))),
            },
        }
    }
    Ok(())
}

/// What a detach says. It names the sha because that is what makes the note checkable — the
/// worktree is where it was, and here is the commit to prove it — and the remedy because
/// "detached HEAD" is a state people meet rarely and by accident.
fn detached(held: &crate::git::worktree::Holder, branch: &str) -> String {
    let sha = held.head.get(..7).unwrap_or(&held.head);
    format!("note: detached {} from {branch} at {sha}, so the branch could move; `git checkout -` there re-attaches it", held.path.display())
}

/// What a worktree left on a branch that moved anyway says.
fn left_on_it(held: &crate::git::worktree::Holder, branch: &str, why: &str) -> String {
    format!(
        "warning: {} has {branch} checked out and {why}, so it was left on it; it now disagrees with the branch — `git reset --hard` there",
        held.path.display()
    )
}

/// What a worktree mid-operation says — the one case that refuses the write outright.
fn refusal(held: &crate::git::worktree::Holder, branch: &str, what: &str) -> String {
    format!("{} has {branch} checked out with {what} in progress; finish or abort it there before writing to the tracker", held.path.display())
}

/// A changeset path as git spells it.
///
/// Changeset paths are built with `PathBuf::join`, so on Windows they arrive with backslashes
/// — and a tree entry named `items\a-a.md` is one path component containing a backslash, not
/// a file in `items/`. The tracker would still round-trip through itself and be unreadable to
/// everything else, which is the kind of bug that only shows up on the platform nobody
/// developing it runs.
fn git_path(path: &std::path::Path) -> String {
    path.components().filter_map(|c| c.as_os_str().to_str()).collect::<Vec<&str>>().join("/")
}

#[cfg(test)]
mod tests {
    // Tests assert; that is their job. The crate denies unwrap/expect/panic because a
    // malformed tracker must produce a diagnostic rather than a stack trace, but a test
    // that cannot panic cannot fail.
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;
    use std::path::PathBuf;

    /// Forward slashes whatever the platform's separator is — a tree entry is a git path,
    /// not a local one.
    #[test]
    fn a_changeset_path_becomes_a_git_path() {
        assert_eq!(git_path(&PathBuf::from("index.jsonl")), "index.jsonl");
        assert_eq!(git_path(&PathBuf::from("items").join("aaaaaaa-a.md")), "items/aaaaaaa-a.md");
    }

    /// Writes land on `refs/heads/` whatever they were read from — a remote-tracking ref is
    /// a copy of someone else's branch, and moving it locally makes this clone lie about it.
    #[test]
    fn a_write_targets_the_local_branch() {
        assert_eq!(local_ref("trck-issues"), "refs/heads/trck-issues");
        assert_eq!(local_ref("origin/trck-issues"), "refs/heads/trck-issues");
        assert_eq!(local_ref("refs/heads/trck-issues"), "refs/heads/trck-issues");
    }

    /// The same branch, in the spelling a revision is read and shown in — so a clone naming a
    /// body before it has the branch says what a clone that already has it says.
    #[test]
    fn the_branch_is_named_without_its_ref_prefix() {
        assert_eq!(local_branch("trck-issues"), "trck-issues");
        assert_eq!(local_branch("origin/trck-issues"), "trck-issues");
        assert_eq!(local_branch("refs/heads/trck-issues"), "trck-issues");
    }
}

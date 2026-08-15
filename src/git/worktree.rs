//! Who else has this branch checked out, and getting them off it before it moves.
//!
//! [`refs::update_ref`](super::refs::update_ref) is plumbing, and plumbing has no worktree
//! guard: the check that refuses to move a branch somebody has checked out lives in porcelain
//! — `branch -f`, `switch`, `worktree add`, `fetch` — and nowhere near `update-ref`. So a
//! tracker write moves the branch under a live checkout without a word, and what it leaves
//! behind is worse than a refusal would have been: that worktree's `HEAD` is now a commit its
//! index and working tree know nothing about, so `git status` there reports the write
//! *inverted* — the body just filed reads as a staged deletion — and an ordinary `git commit
//! -a` turns that reading into a commit which reverts the write and pushes as a fast-forward.
//! No conflict, no rejection, no row.
//!
//! **So the checkout is made honest instead.** [`detach`] puts it on the commit it was already
//! sitting on: `update-ref --no-deref HEAD <sha>` overwrites the symbolic ref itself rather than
//! what it points at, so the index and the working tree are neither read nor written, a dirty
//! tree is fine, and the per-worktree reflog gets an entry — `git checkout -` re-attaches. What
//! that worktree holds does not change. Only whether a branch name still follows it does.
//!
//! This is the one place the engine reaches into a checkout it does not own, which is the
//! property the ref-backed tracker was argued for. It is a narrow reach — a symref, never
//! content, never the index — and the alternative is not "nothing happens", it is a silent
//! desync that eats a tracker write.
//!
//! What this file does *not* hold is the policy: which holder is left alone, which one stops a
//! write, and what any of it says to the operator. That is
//! [`verbs::backend::release`](crate::verbs::backend::release), because the errors here are
//! unphrased like everything else under [`super`] — this module reports what git says about the
//! repository, and the layer above decides what it means.

use super::stdout;
use State::{Busy, Free, Locked};
use std::path::{Path, PathBuf};

/// A worktree with the branch in question checked out.
pub(crate) struct Holder {
    /// Where it is on disk, which is also how it is named in a diagnostic.
    pub(crate) path: PathBuf,
    /// The commit it is sitting on — and, after a detach, still sitting on.
    pub(crate) head: String,
    pub(crate) state: State,
}

/// What may be done to a holder.
pub(crate) enum State {
    /// Detachable.
    Free,
    /// Its owner has said not to touch it: removable media, or a checkout being kept from
    /// `prune`. A lock is not about ref moves, but it is the only signal anyone gets to leave.
    Locked,
    /// Mid-operation, named for the diagnostic.
    Busy(&'static str),
}

/// Is anybody sitting on `refname`?
///
/// What a *read* asks. A read may fast-forward the local branch, and doing that to a checkout
/// is the same desync a write would cause, arriving from `trck list` — so the read declines to
/// move it and answers from the remote-tracking ref instead, which holds the same commits.
pub(crate) fn is_checked_out(cwd: &Path, refname: &str) -> Result<bool, String> {
    Ok(!holders(cwd, refname)?.is_empty())
}

/// Every worktree of this repository that has `refname` checked out.
///
/// A worktree whose directory is gone is not one of them: it cannot commit, so there is
/// nothing there to protect and nothing worth saying about it.
pub(crate) fn holders(cwd: &Path, refname: &str) -> Result<Vec<Holder>, String> {
    let listing = stdout(cwd, &["worktree", "list", "--porcelain"])?;
    Ok(listing.split("\n\n").filter_map(|record| holder(record, refname)).collect())
}

/// One porcelain record, if it is a holder of `refname`.
///
/// The format is one `key value` per line — `worktree <path>`, `HEAD <sha>`, `branch <ref>` —
/// with `detached`, `bare`, `locked` and `prunable` standing alone or carrying a reason.
fn holder(record: &str, refname: &str) -> Option<Holder> {
    let lines: Vec<(&str, &str)> = record.lines().map(|line| line.split_once(' ').unwrap_or((line, ""))).collect();
    let field = |key: &str| lines.iter().find(|(k, _)| *k == key).map(|(_, value)| *value);
    if field("prunable").is_some() || field("branch") != Some(refname) {
        return None;
    }
    let path = PathBuf::from(field("worktree")?);
    let state = if field("locked").is_some() { Locked } else { in_progress(&path).map_or(Free, Busy) };
    Some(Holder { path, head: field("HEAD")?.to_string(), state })
}

/// What this worktree is in the middle of, if anything.
///
/// Every one of these records where `HEAD` was when it started — a rebase replays onto it, a
/// merge concludes on it — so rewriting `HEAD` out from under one leaves an operator holding a
/// conflict they can no longer finish. That is worth refusing a tracker write over; the row
/// can be filed a minute later, and the rebase cannot be un-stranded.
fn in_progress(path: &Path) -> Option<&'static str> {
    let dir = PathBuf::from(stdout(path, &["rev-parse", "--absolute-git-dir"]).ok()?);
    let markers = [
        ("rebase-merge", "a rebase"),
        ("rebase-apply", "a rebase"),
        ("MERGE_HEAD", "a merge"),
        ("CHERRY_PICK_HEAD", "a cherry-pick"),
        ("REVERT_HEAD", "a revert"),
        ("BISECT_LOG", "a bisect"),
    ];
    markers.into_iter().find(|(marker, _)| dir.join(marker).exists()).map(|(_, what)| what)
}

/// Turn this worktree's `HEAD` from a branch name into the commit it already holds.
///
/// `--no-deref` is the whole trick: without it the update follows the symbolic ref and moves
/// the *branch*, which is the opposite of the point.
pub(crate) fn detach(path: &Path, head: &str) -> Result<(), String> {
    stdout(path, &["update-ref", "--no-deref", "HEAD", head]).map(|_| ())
}

#[cfg(test)]
mod tests {
    // Tests assert; that is their job. See the note in `discovery::tests`.
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;
    use crate::git::rev_parse;
    use crate::git::tests::{commit_file, repo};
    use crate::git::{refs::update_ref, run};

    const TRACKER: &str = "refs/heads/trck-issues";

    /// A repository with a commit, a `trck-issues` branch, and a worktree sitting on it.
    ///
    /// The worktree goes inside the repository directory rather than beside it: `Tmp` removes
    /// what it made and nothing else, and a sibling in the system temp directory would outlive
    /// the test.
    fn with_worktree(tag: &str) -> Option<(crate::discovery::tests::Tmp, PathBuf, PathBuf)> {
        let (tmp, dir) = repo(tag)?;
        commit_file(&dir, "f", "A\n");
        stdout(&dir, &["branch", "trck-issues"]).expect("branch");
        let wt = dir.join("wt");
        stdout(&dir, &["worktree", "add", "-q", &wt.display().to_string(), "trck-issues"]).expect("worktree");
        Some((tmp, dir, wt))
    }

    #[test]
    fn a_worktree_on_the_branch_is_found_with_the_commit_it_holds() {
        let Some((_tmp, dir, wt)) = with_worktree("wt-holders") else { return };
        let held = holders(&dir, TRACKER).expect("listed");
        assert_eq!(held.len(), 1, "one worktree has it checked out");
        assert_eq!(held[0].path.canonicalize().ok(), wt.canonicalize().ok());
        assert_eq!(Some(held[0].head.clone()), rev_parse(&dir, "trck-issues").expect("sha"));
    }

    /// The main worktree is on `main`, and a detached one follows no branch at all. Neither
    /// can be desynced by moving the tracker branch, so neither is anybody's business.
    #[test]
    fn a_worktree_on_another_branch_or_on_no_branch_is_not_a_holder() {
        let Some((_tmp, dir, wt)) = with_worktree("wt-others") else { return };
        stdout(&wt, &["checkout", "-q", "--detach"]).expect("detach");
        assert!(holders(&dir, TRACKER).expect("listed").is_empty(), "a detached worktree still counts as holding the branch");
        assert_eq!(holders(&dir, "refs/heads/main").expect("listed").len(), 1, "the main worktree is on main");
    }

    /// A worktree whose directory has been deleted: git still lists it, annotated `prunable`.
    /// Nothing can be committed from it, so there is nothing to detach and nothing to say.
    #[test]
    fn a_worktree_whose_directory_is_gone_is_not_a_holder() {
        let Some((_tmp, dir, wt)) = with_worktree("wt-prunable") else { return };
        std::fs::remove_dir_all(&wt).expect("rm");
        assert!(holders(&dir, TRACKER).expect("listed").is_empty());
    }

    #[test]
    fn a_locked_worktree_is_reported_locked_rather_than_free() {
        let Some((_tmp, dir, wt)) = with_worktree("wt-locked") else { return };
        stdout(&dir, &["worktree", "lock", &wt.display().to_string()]).expect("lock");
        let held = holders(&dir, TRACKER).expect("listed");
        assert!(matches!(held.first().map(|h| &h.state), Some(Locked)), "a locked worktree is not free to detach");
    }

    /// A conflicted merge, not a hand-made marker file: what is being asserted is that git's
    /// own idea of "mid-operation" is what gets read.
    #[test]
    fn a_worktree_with_a_merge_in_progress_is_busy() {
        let Some((_tmp, dir, wt)) = with_worktree("wt-busy") else { return };
        commit_file(&wt, "f", "B\n");
        commit_file(&dir, "f", "C\n");
        let merge = run(&wt, &["merge", "main"]).expect("ran");
        assert!(!merge.status.success(), "the merge was supposed to conflict");

        let held = holders(&dir, TRACKER).expect("listed");
        assert!(matches!(held.first().map(|h| &h.state), Some(Busy("a merge"))), "a conflicted merge is not reported as in progress");
    }

    /// The whole mechanism, in one assertion: after the detach the worktree is on the same
    /// commit with the same working tree, and the branch it used to follow can move without
    /// dragging it along.
    #[test]
    fn detaching_leaves_the_worktree_exactly_where_it_was() {
        let Some((_tmp, dir, wt)) = with_worktree("wt-detach") else { return };
        std::fs::write(wt.join("f"), "half-edited\n").expect("write");
        let was = rev_parse(&dir, "trck-issues").expect("sha").expect("a branch");

        detach(&wt, &was).expect("detach");

        assert!(!run(&wt, &["symbolic-ref", "-q", "HEAD"]).expect("ran").status.success(), "HEAD still follows a branch");
        assert_eq!(rev_parse(&wt, "HEAD").expect("sha"), Some(was.clone()), "the worktree moved");
        assert_eq!(std::fs::read_to_string(wt.join("f")).expect("read"), "half-edited\n", "the working tree was touched");

        let moved = commit_file(&dir, "g", "on main\n");
        update_ref(&dir, TRACKER, &moved, Some(&was)).expect("move the branch");
        assert_eq!(rev_parse(&wt, "HEAD").expect("sha"), Some(was), "the branch move dragged the worktree with it");
    }
}

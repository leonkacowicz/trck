//! Moving refs, locally and on a remote.
//!
//! Split from [`super::write`] because building an object and moving a ref are different acts
//! with different consequences: an object nothing points at is garbage a `gc` collects, while
//! a ref move is what makes a change real — and what another writer can lose to.
//!
//! **Every move here is a compare-and-swap.** [`update_ref`] takes the value the caller
//! believes the ref currently holds and git refuses the move if it holds anything else;
//! [`push`] moves a remote ref only when the move is a fast-forward, which is the same
//! guarantee enforced by the remote. There is no unconditional form of either, and no
//! `--force` anywhere in this file: a rejection means someone else's work landed, and the
//! answer to that is to re-read and retry, never to overwrite.

use super::stdout;
use std::path::Path;

/// Move `name` to `new`, but only if it currently holds `old`.
///
/// `None` means the ref must not exist yet — the first write to a tracker branch. There is
/// deliberately no "move it regardless" form: every caller knows what it read, and a move
/// from an unread value is a lost write.
pub(crate) fn update_ref(cwd: &Path, name: &str, new: &str, old: Option<&str>) -> Result<(), String> {
    stdout(cwd, &["update-ref", name, new, old.unwrap_or("")]).map(|_| ())
}

/// Fetch `refname` from `remote` into its remote-tracking ref.
///
/// Explicit rather than a bare `fetch`: this runs on the rejection path of someone else's
/// tracker write, and dragging down every branch in the repository would make a contended
/// `trck done` cost whatever the rest of the remote happens to weigh.
pub(crate) fn fetch(cwd: &Path, remote: &str, refname: &str) -> Result<(), String> {
    let spec = format!("+{refname}:refs/remotes/{remote}/{}", refname.trim_start_matches("refs/heads/"));
    stdout(cwd, &["fetch", "--quiet", remote, &spec]).map(|_| ())
}

/// Is `remote` configured here?
///
/// A tracker with no remote is a legitimate tracker — a local repository, or one whose branch
/// has never been shared. It is not an error, and a write to it must not fail for want of
/// somewhere to push; it simply has nothing to do.
pub(crate) fn has_remote(cwd: &Path, remote: &str) -> bool {
    super::run(cwd, &["config", "--get", &format!("remote.{remote}.url")]).is_ok_and(|o| o.status.success())
}

/// Push `sha` to `refname` on `remote`.
///
/// No refspec `+`, no `--force`, no `--force-with-lease`: a plain push of a sha to a branch
/// ref is already a compare-and-swap, since the remote rejects anything that is not a
/// fast-forward of what it holds. That rejection is the whole concurrency control — it is
/// what makes it safe to build a commit against a possibly-stale base, because a commit
/// whose parent is not the current remote tip cannot land.
pub(crate) fn push(cwd: &Path, remote: &str, sha: &str, refname: &str) -> Result<(), String> {
    stdout(cwd, &["push", remote, &format!("{sha}:{refname}")]).map(|_| ())
}

#[cfg(test)]
mod tests {
    // Tests assert; that is their job. See the note in `discovery::tests`.
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;
    use crate::git::rev_parse;
    use crate::git::tests::repo;
    use crate::git::write::{commit_tree, hash_object, write_tree};

    /// A tracker-shaped commit anchored at `refname` — the starting point for every question
    /// about moving a ref off it.
    fn anchored(dir: &Path, refname: &str, index: &str) -> String {
        let blob = hash_object(dir, index).expect("blob");
        let tree = write_tree(dir, &[("index.jsonl", &blob)]).expect("tree");
        let commit = commit_tree(dir, &tree, &[], "new #a: a\n\nTrck-Op: new a\n").expect("commit");
        update_ref(dir, refname, &commit, None).expect("create ref");
        commit
    }

    /// A second commit on top of `parent`, ready to move a ref onto.
    fn successor(dir: &Path, parent: &str, index: &str) -> String {
        let blob = hash_object(dir, index).expect("blob");
        let tree = write_tree(dir, &[("index.jsonl", &blob)]).expect("tree");
        commit_tree(dir, &tree, &[parent], "set #a\n").expect("commit")
    }

    #[test]
    fn a_ref_move_from_a_value_the_ref_no_longer_holds_is_refused() {
        let Some((_tmp, dir)) = repo("refs-cas") else { return };
        let first = anchored(&dir, "refs/heads/trck-issues", "{\"id\": \"a\"}\n");
        let second = successor(&dir, &first, "{\"id\": \"b\"}\n");
        let stale = "0".repeat(40);
        update_ref(&dir, "refs/heads/trck-issues", &second, Some(&stale)).expect_err("stale expectation");
        // Refused, not partially applied: the ref still holds what it held.
        assert_eq!(rev_parse(&dir, "refs/heads/trck-issues").unwrap(), Some(first));
    }

    #[test]
    fn creating_a_ref_that_already_exists_is_refused() {
        let Some((_tmp, dir)) = repo("refs-create") else { return };
        let first = anchored(&dir, "refs/heads/trck-issues", "{\"id\": \"a\"}\n");
        let second = successor(&dir, &first, "{\"id\": \"b\"}\n");
        update_ref(&dir, "refs/heads/trck-issues", &second, None).expect_err("already exists");
    }

    #[test]
    fn a_push_lands_a_fast_forward_and_is_rejected_otherwise() {
        let Some((tmp, dir)) = repo("refs-push") else { return };
        let remote = tmp.path().join("remote.git");
        let Some(remote_path) = remote.to_str() else { return };
        stdout(&dir, &["init", "-q", "--bare", remote_path]).expect("bare remote");
        let first = anchored(&dir, "refs/heads/trck-issues", "{\"id\": \"a\"}\n");
        push(&dir, remote_path, &first, "refs/heads/trck-issues").expect("first push");
        assert_eq!(stdout(&remote, &["rev-parse", "refs/heads/trck-issues"]).unwrap(), first);

        // A commit built against no parent is not a fast-forward of what the remote holds:
        // exactly the shape of a write that raced someone else's.
        let blob = hash_object(&dir, "{\"id\": \"b\"}\n").expect("blob");
        let tree = write_tree(&dir, &[("index.jsonl", &blob)]).expect("tree");
        let unrelated = commit_tree(&dir, &tree, &[], "new #b: b\n").expect("commit");
        push(&dir, remote_path, &unrelated, "refs/heads/trck-issues").expect_err("non-fast-forward");
        assert_eq!(stdout(&remote, &["rev-parse", "refs/heads/trck-issues"]).unwrap(), first);
    }

    /// A fetch brings the remote's branch back under a remote-tracking name, without touching
    /// the local branch of the same name — which the rebuild path then moves deliberately.
    #[test]
    fn a_fetch_updates_the_tracking_ref_and_leaves_the_local_branch_alone() {
        let Some((tmp, dir)) = repo("refs-fetch") else { return };
        let remote = tmp.path().join("remote.git");
        let Some(remote_path) = remote.to_str() else { return };
        stdout(&dir, &["init", "-q", "--bare", remote_path]).expect("bare remote");
        stdout(&dir, &["remote", "add", "origin", remote_path]).expect("add remote");
        let first = anchored(&dir, "refs/heads/trck-issues", "{\"id\": \"a\"}\n");
        push(&dir, "origin", &first, "refs/heads/trck-issues").expect("push");
        // The remote moves on without us: a commit pushed straight there, so the local branch
        // is left holding `first` — exactly the state a rejected write is rebuilt from.
        let second = successor(&dir, &first, "{\"id\": \"b\"}\n");
        push(&dir, "origin", &second, "refs/heads/trck-issues").expect("their push");

        fetch(&dir, "origin", "refs/heads/trck-issues").expect("fetch");
        assert_eq!(rev_parse(&dir, "refs/remotes/origin/trck-issues").unwrap(), Some(second));
        assert_eq!(rev_parse(&dir, "refs/heads/trck-issues").unwrap(), Some(first), "the fetch moved the local branch");
    }

    /// A repository with no such remote configured. Not an error to ask — a tracker that has
    /// never been shared is a legitimate tracker, and the write path branches on this rather
    /// than failing.
    #[test]
    fn a_remote_that_is_not_configured_is_answered_no() {
        let Some((_tmp, dir)) = repo("refs-noremote") else { return };
        assert!(!has_remote(&dir, "origin"));
    }
}

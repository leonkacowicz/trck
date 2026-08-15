//! Writing a branch somebody has checked out.
//!
//! Nothing stops an operator from `git worktree add`-ing the tracker branch, and git will not
//! defend that checkout from us: `update-ref` is plumbing, and the guard that refuses to move a
//! checked-out branch lives only in porcelain. So the move lands, the worktree's `HEAD` jumps to
//! a commit its index and working tree know nothing about, and `git status` there shows the
//! tracker write *inverted* — from which an ordinary `git commit -a` reverts it and pushes as a
//! clean fast-forward.
//!
//! The answer is to make that checkout honest rather than to refuse the write: detach it at the
//! commit it was actually on, first, so the branch moves out from under nothing. What it holds
//! never changes — only whether a branch name still follows it.
//!
//! Reads are the other half. A read may fast-forward the local branch (`#abynj5c`), and doing
//! *that* to a checkout would be the same desync arriving from `trck list`. So a read does not
//! detach anybody: it leaves the branch alone and answers from `origin/trck-issues` instead,
//! which is the same content the fast-forward would have produced.

// Tests assert; that is their job. The crate denies unwrap/expect/panic because a malformed
// tracker must produce a diagnostic rather than a stack trace, but a test that cannot panic
// cannot fail.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::{Scenario, TRACKER_BRANCH, git_must, git_ok, trck, trck_must};
use std::path::{Path, PathBuf};

/// The issue the seeded second commit adds — what reading the remote's tip proves.
const SECOND: &str = "bbbbbbb";

fn sha(dir: &Path, rev: &str) -> String {
    git_must(dir, &["rev-parse", rev])
}

/// Put the local branch at `rev` without checking anything out.
fn point_local_at(work: &Path, rev: &str) {
    let target = sha(work, rev);
    git_must(work, &["update-ref", &format!("refs/heads/{TRACKER_BRANCH}"), &target]);
}

/// A second worktree of the clone, sitting on the tracker branch.
///
/// Two levels down rather than beside the clone. A checkout of the tracker branch *is* a
/// tracker directory — that is what the branch holds — and discovery scans each ancestor's
/// children as well as the ancestors themselves, so a worktree parked next to `work` would be
/// resolved as a directory tracker and read instead of the ref this whole file is about.
fn holder(work: &Path, name: &str) -> PathBuf {
    let under = work.parent().expect("the clone has a parent").join("holders");
    std::fs::create_dir_all(&under).expect("mkdir");
    let at = under.join(name);
    git_must(work, &["worktree", "add", "-q", &at.display().to_string(), TRACKER_BRANCH]);
    at
}

/// Is this worktree still following a branch, rather than sitting on a commit?
fn attached(dir: &Path) -> bool {
    git_ok(dir, &["symbolic-ref", "-q", "HEAD"])
}

/// What the verb said on stderr — where every note about somebody else's worktree belongs, so
/// that piped output stays parseable.
fn said(out: &std::process::Output) -> String {
    String::from_utf8_lossy(&out.stderr).to_string()
}

/// Does this diagnostic name that worktree?
///
/// Separators normalised and case folded, because the two sides spell a Windows path
/// differently: `git worktree list` answers `C:/Users/…` and `Path::display` writes
/// `C:\Users\…`. The engine repeats git's spelling rather than inventing one of its own —
/// that is the string an operator can paste straight back into a shell — so it is the *test*
/// that has to meet it, not the message.
fn names(said: &str, path: &Path) -> bool {
    let flat = |s: &str| s.replace('\\', "/").to_lowercase();
    flat(said).contains(&flat(&path.display().to_string()))
}

/// The core of it: the branch moves, and the checkout that held it is left where it was.
#[test]
fn a_write_detaches_a_worktree_that_holds_the_tracker_branch() {
    let Some(s) = Scenario::build("wt-detach") else {
        return;
    };
    point_local_at(&s.work, &format!("origin/{TRACKER_BRANCH}"));
    let held = holder(&s.work, "holder");
    let was = sha(&held, "HEAD");

    let out = trck(&s.work, &["new", "Filed while the branch was checked out", "--id", "ccccccc", "--empty"]);
    assert!(out.status.success(), "the write failed: {}", said(&out));

    assert_ne!(sha(&s.work, TRACKER_BRANCH), was, "the branch did not move, so nothing was written");
    assert_eq!(sha(&held, "HEAD"), was, "the worktree was dragged onto a commit it knows nothing about");
    assert!(!attached(&held), "still following the branch that moved");

    let err = said(&out);
    assert!(err.contains("detached"), "the detach was silent: {err}");
    assert!(names(&err, &held), "the note does not name the worktree: {err}");
}

/// Detaching is a symref rewrite and nothing else. Half-written prose in that worktree is
/// somebody's work in progress, and a tracker write is not entitled to touch it.
#[test]
fn a_detached_worktree_keeps_its_index_and_working_tree() {
    let Some(s) = Scenario::build("wt-dirty") else {
        return;
    };
    point_local_at(&s.work, &format!("origin/{TRACKER_BRANCH}"));
    let held = holder(&s.work, "holder");
    std::fs::write(held.join("index.jsonl"), "half-edited by hand\n").expect("write");
    std::fs::write(held.join("scratch.md"), "notes\n").expect("write");
    let before = git_must(&held, &["status", "--porcelain"]);

    let out = trck(&s.work, &["new", "Filed over a dirty worktree", "--id", "ccccccc", "--empty"]);
    assert!(out.status.success(), "the write failed: {}", said(&out));

    assert_eq!(git_must(&held, &["status", "--porcelain"]), before, "the working tree or index moved");
    assert_eq!(std::fs::read_to_string(held.join("index.jsonl")).expect("read"), "half-edited by hand\n");
    assert_eq!(std::fs::read_to_string(held.join("scratch.md")).expect("read"), "notes\n");
}

/// A read may fast-forward the local branch, and that move would desync a checkout exactly the
/// way a write's does. So it does not make it: it answers from the remote-tracking ref, which
/// holds the same commits the fast-forward would have brought.
#[test]
fn a_read_does_not_move_the_branch_while_it_is_checked_out() {
    let Some(s) = Scenario::build("wt-read") else {
        return;
    };
    point_local_at(&s.work, &format!("origin/{TRACKER_BRANCH}~1"));
    let held = holder(&s.work, "holder");
    let was = sha(&s.work, TRACKER_BRANCH);

    let listed = trck_must(&s.work, &["list"]);
    assert!(listed.contains(SECOND), "the read did not answer from the remote:\n{listed}");
    assert_eq!(sha(&s.work, TRACKER_BRANCH), was, "a read moved a checked-out branch");
    assert!(attached(&held), "a read detached somebody's worktree");
    assert_eq!(sha(&held, "HEAD"), was, "a read moved somebody's HEAD");
}

/// The write that follows such a read must not build on the ref the read declined to move —
/// that commit's parent is the remote's tip, so the branch ends up where the fast-forward
/// would have put it, plus the new work.
#[test]
fn a_write_from_behind_a_checked_out_branch_keeps_the_remote_history() {
    let Some(s) = Scenario::build("wt-behind") else {
        return;
    };
    point_local_at(&s.work, &format!("origin/{TRACKER_BRANCH}~1"));
    let held = holder(&s.work, "holder");
    let was = sha(&held, "HEAD");
    let remote = sha(&s.work, &format!("origin/{TRACKER_BRANCH}"));

    let out = trck(&s.work, &["new", "Filed from behind", "--id", "ccccccc", "--empty"]);
    assert!(out.status.success(), "the write failed: {}", said(&out));

    assert!(git_ok(&s.work, &["merge-base", "--is-ancestor", &remote, TRACKER_BRANCH]), "the write dropped what the remote held");
    let listed = trck_must(&s.work, &["list"]);
    assert!(listed.contains(SECOND) && listed.contains("ccccccc"), "{listed}");
    assert_eq!(sha(&held, "HEAD"), was, "the worktree moved");
    assert!(!attached(&held), "still following the branch that moved");
}

/// A worktree mid-merge has state that a detach would strand: `MERGE_HEAD` names a commit the
/// conclusion needs, and rewriting `HEAD` out from under it leaves an operator with a conflict
/// they can no longer finish. Refuse, and say where.
#[test]
fn a_worktree_with_a_merge_in_progress_refuses_the_write() {
    let Some(s) = Scenario::build("wt-busy") else {
        return;
    };
    point_local_at(&s.work, &format!("origin/{TRACKER_BRANCH}~1"));
    let held = holder(&s.work, "holder");

    // A commit of its own on the branch, touching a file the remote's tip also touched, so
    // that merging the remote in is a real conflict rather than a fast-forward. `SUMMARY.md`
    // rather than `index.jsonl`: what is under test is the refusal, and a hand-mangled index
    // would fail the read long before the write path is reached.
    std::fs::write(held.join("SUMMARY.md"), "# hand-edited in the worktree\n").expect("write");
    git_must(&held, &["commit", "-qam", "a hand edit on the tracker branch"]);
    assert!(!git_ok(&held, &["merge", &format!("origin/{TRACKER_BRANCH}")]), "the merge was supposed to conflict");
    assert!(held.join(".git").exists(), "a linked worktree has a .git file");
    let was = sha(&s.work, TRACKER_BRANCH);

    let out = trck(&s.work, &["new", "Filed onto a busy worktree", "--id", "ccccccc", "--empty"]);
    assert!(!out.status.success(), "the write should have been refused");
    let err = said(&out);
    assert!(names(&err, &held), "the refusal does not name the worktree: {err}");
    assert!(err.contains("merge"), "the refusal does not say what is in progress: {err}");
    assert_eq!(sha(&s.work, TRACKER_BRANCH), was, "refused, and yet the branch moved");
}

/// A locked worktree is one its owner has said not to touch — removable media, or a checkout
/// somebody is protecting from `prune`. The write still lands, because a tracker write is not
/// blocked by somebody else's disk, but the desync it leaves is named rather than silent.
#[test]
fn a_locked_worktree_is_warned_about_and_the_write_lands() {
    let Some(s) = Scenario::build("wt-locked") else {
        return;
    };
    point_local_at(&s.work, &format!("origin/{TRACKER_BRANCH}"));
    let held = holder(&s.work, "holder");
    let was = sha(&held, "HEAD");
    git_must(&s.work, &["worktree", "lock", &held.display().to_string()]);

    let out = trck(&s.work, &["new", "Filed past a locked worktree", "--id", "ccccccc", "--empty"]);
    assert!(out.status.success(), "a locked worktree is not a failed write: {}", said(&out));

    assert_ne!(sha(&s.work, TRACKER_BRANCH), was, "the branch did not move");
    assert!(attached(&held), "a locked worktree was detached anyway");
    let err = said(&out);
    assert!(err.contains("locked"), "the desync was silent: {err}");
    assert!(names(&err, &held), "the warning does not name the worktree: {err}");
}

/// A worktree whose directory is gone cannot commit anything, so there is nothing to protect
/// and nothing worth saying about it.
#[test]
fn a_worktree_whose_directory_is_gone_is_ignored() {
    let Some(s) = Scenario::build("wt-prunable") else {
        return;
    };
    point_local_at(&s.work, &format!("origin/{TRACKER_BRANCH}"));
    let held = holder(&s.work, "holder");
    let was = sha(&s.work, TRACKER_BRANCH);
    std::fs::remove_dir_all(&held).expect("rm the worktree");

    let out = trck(&s.work, &["new", "Filed past a vanished worktree", "--id", "ccccccc", "--empty"]);
    assert!(out.status.success(), "the write failed: {}", said(&out));

    assert_ne!(sha(&s.work, TRACKER_BRANCH), was, "the branch did not move");
    let err = said(&out);
    assert!(!err.contains("detached"), "there was nothing there to detach: {err}");
    assert!(!err.contains("locked"), "{err}");
}

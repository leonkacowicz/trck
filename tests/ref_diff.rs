//! `trck diff` when the tracker's history is not the checkout's.
//!
//! `diff` is the one read verb whose *meaning* changes rather than just its source. Every
//! other verb reads the tracker as it stands now, and the ref layer only moved where "now"
//! comes from. `diff` reads it at a revision — and once the tracker is on its own branch,
//! `HEAD~5` in the checkout is five commits of code and says nothing about any issue.
//!
//! The failure to avoid is not an error; it is a confident wrong answer. `git show
//! <a-code-commit>:index.jsonl` finds nothing, and nothing renders as an empty tracker,
//! which renders as "every issue is new".

// Tests assert; that is their job. The crate denies unwrap/expect/panic because a malformed
// tracker must produce a diagnostic rather than a stack trace, but a test that cannot panic
// cannot fail.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::{Scenario, TRACKER_BRANCH, git_must, trck, trck_must};

/// `HEAD` is the tracker branch's tip, so `HEAD~1` is the commit before the second issue was
/// filed — not whatever the checkout happens to be sitting on.
#[test]
fn head_counts_tracker_commits_not_the_checkouts() {
    let Some(s) = Scenario::build("diff-head") else {
        return;
    };
    let out = trck_must(&s.work, &["diff", "HEAD~1..HEAD"]);
    assert!(out.contains("bbbbbbb"), "the second issue should be what changed:\n{out}");
    assert!(!out.contains("aaaaaaa"), "the first issue was already there:\n{out}");
}

/// The whole history of the tracker, which on the checkout's `HEAD` would be nothing at all.
#[test]
fn a_diff_against_the_first_tracker_commit_sees_the_second_issue() {
    let Some(s) = Scenario::build("diff-first") else {
        return;
    };
    let first = git_must(&s.work, &["rev-parse", &format!("origin/{TRACKER_BRANCH}~1")]);
    let out = trck_must(&s.work, &["diff", &format!("{first}..origin/{TRACKER_BRANCH}")]);
    assert!(out.contains("bbbbbbb"), "{out}");
}

/// The confident wrong answer this exists to prevent: a revision of the *code* holds no
/// index, and an empty index reads as "every issue is new".
#[test]
fn a_revision_that_is_not_a_tracker_revision_is_an_error_not_an_empty_diff() {
    let Some(s) = Scenario::build("diff-wrong-branch") else {
        return;
    };
    let out = trck(&s.work, &["diff", "origin/main..origin/main"]);
    assert!(!out.status.success(), "a code revision was diffed as if it were a tracker: {out:?}");
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("no index.jsonl"), "unexpected wording: {err}");
    assert!(err.contains("origin/main"), "the refusal must name the revision: {err}");
}

/// A revision that does not exist keeps saying so — that is a different mistake from naming
/// a real commit that holds no tracker.
#[test]
fn an_unknown_revision_still_says_unknown() {
    let Some(s) = Scenario::build("diff-unknown") else {
        return;
    };
    let out = trck(&s.work, &["diff", "no-such-rev..HEAD"]);
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("unknown revision"), "{err}");
    assert!(err.contains("no-such-rev"), "{err}");
}

/// `--from` names a file rather than a revision, and none of this touches it.
#[test]
fn from_a_file_is_unaffected() {
    let Some(s) = Scenario::build("diff-from-file") else {
        return;
    };
    let snapshot = s.work.join("was.jsonl");
    let index = s.show(&format!("origin/{TRACKER_BRANCH}~1"), "index.jsonl").expect("the earlier index");
    std::fs::write(&snapshot, index).expect("write");

    let out = trck_must(&s.work, &["diff", "--from", &snapshot.display().to_string()]);
    assert!(out.contains("bbbbbbb"), "the second issue is what arrived since:\n{out}");
}

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

/// The header names what was actually compared, and here that is not a working tree.
///
/// `diff`'s right-hand side is "the tracker as it stands", which for a directory is the files
/// on disk and is labelled `working tree`. Read out of a ref there is no working tree in the
/// comparison at all — the label was describing the wrong thing, and describing it in a repo
/// where a working tree exists but has nothing to do with the answer, which is worse than a
/// name for something absent.
///
/// It also disambiguates the left-hand side. `HEAD~1` was reanchored to the tracker branch, so
/// a reader who takes `HEAD` at its word is counting the wrong commits; naming the branch on
/// the right is the cheapest available correction.
#[test]
fn the_right_hand_side_names_the_ref_not_a_working_tree() {
    let Some(s) = Scenario::build("diff-label-ref") else {
        return;
    };
    // No local branch yet, so the tracker resolves through the remote-tracking ref — and the
    // label has to be the ref that was *read*, whichever of the two spellings that is.
    let out = trck_must(&s.work, &["diff", "HEAD~1"]);

    let header = out.lines().next().unwrap_or_default();
    assert_eq!(header, format!("HEAD~1 → origin/{TRACKER_BRANCH}"), "unexpected header in:\n{out}");
    assert!(!out.contains("working tree"), "there is no working tree in this comparison:\n{out}");
}

/// And once the clone has its own tracker branch, that is the one named.
#[test]
fn the_label_follows_the_ref_that_was_resolved() {
    let Some(s) = Scenario::build("diff-label-local") else {
        return;
    };
    // A write creates the local branch, after which resolution prefers it over the remote.
    trck_must(&s.work, &["new", "Filed here", "--id", "ccccccc", "--empty"]);

    let out = trck_must(&s.work, &["diff", "HEAD~1"]);

    let header = out.lines().next().unwrap_or_default();
    assert_eq!(header, format!("HEAD~1 → {TRACKER_BRANCH}"), "unexpected header in:\n{out}");
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

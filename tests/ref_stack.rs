//! Replaying a *stack* of pending commits, not just the one this process made.
//!
//! At depth one the operation is still in hand, which is why it was passed in. Once commits
//! stack — three issues filed offline, then the remote moves — the earlier operations are
//! not in memory, and the `Trck-Op:` trailer is the only record of them. Replaying only the
//! last would silently drop the rest, which is the quietest way a tracker can lose work.
//!
//! The other half is refusal. An op that no longer applies is a real conflict and belongs in
//! front of a human; what must not happen is a half-replayed stack, holding some of what the
//! operator did and none of the rest, with nothing saying which.

// Tests assert; that is their job. The crate denies unwrap/expect/panic because a malformed
// tracker must produce a diagnostic rather than a stack trace, but a test that cannot panic
// cannot fail.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::{Scenario, TRACKER_BRANCH, git_must, trck, trck_must};
use std::path::Path;

const LOCAL_REF: &str = "refs/heads/trck-issues";
const UNREACHABLE: &str = "/trck-no-such-remote.git";

fn sha(work: &Path, rev: &str) -> String {
    git_must(work, &["rev-parse", rev])
}

/// File `n` issues with the remote unreachable, leaving a stack of that many pending commits.
fn stack_offline(work: &Path, ids: &[&str]) {
    let url = git_must(work, &["remote", "get-url", "origin"]);
    git_must(work, &["remote", "set-url", "origin", UNREACHABLE]);
    for id in ids {
        let out = trck(work, &["new", &format!("Filed offline {id}"), "--id", id, "--empty"]);
        assert!(out.status.success(), "{out:?}");
    }
    git_must(work, &["remote", "set-url", "origin", &url]);
}

/// Someone else's work lands on the branch while this clone was offline.
fn remote_moves(s: &Scenario, id: &str) {
    let other = s.work.parent().expect("root").join(format!("other-{id}"));
    git_must(s.work.parent().expect("root"), &["clone", "-q", &s.origin.display().to_string(), &format!("other-{id}")]);
    trck_must(&other, &["new", &format!("Filed elsewhere {id}"), "--id", id, "--empty"]);
    let _ = std::fs::remove_dir_all(&other);
}

#[test]
fn a_stack_of_three_replays_in_order_and_converges() {
    let Some(s) = Scenario::build("stack-three") else {
        return;
    };
    stack_offline(&s.work, &["ccccccc", "ddddddd", "eeeeeee"]);
    assert_eq!(git_must(&s.work, &["rev-list", "--count", &format!("origin/{TRACKER_BRANCH}..{LOCAL_REF}")]), "3");

    remote_moves(&s, "fffffff");
    git_must(&s.work, &["fetch", "-q", "origin"]);

    // The next write pushes, is rejected, and rebuilds the whole stack onto what landed.
    trck_must(&s.work, &["new", "The one that triggers it", "--id", "ggggggg", "--empty"]);

    let listed = trck_must(&s.work, &["list"]);
    for id in ["aaaaaaa", "bbbbbbb", "ccccccc", "ddddddd", "eeeeeee", "fffffff", "ggggggg"] {
        assert!(listed.contains(id), "{id} was dropped by the replay:\n{listed}");
    }
    assert_eq!(sha(&s.work, LOCAL_REF), sha(&s.work, &format!("origin/{TRACKER_BRANCH}")), "the stack did not converge");
}

/// Order matters: a later op can act on an issue an earlier one created, and replaying out of
/// order fails on an id that does not exist yet.
#[test]
fn a_stack_that_depends_on_itself_replays_in_the_order_it_was_made() {
    let Some(s) = Scenario::build("stack-order") else {
        return;
    };
    let url = git_must(&s.work, &["remote", "get-url", "origin"]);
    git_must(&s.work, &["remote", "set-url", "origin", UNREACHABLE]);
    trck_must(&s.work, &["new", "Parent", "--id", "ccccccc", "--empty"]);
    // Acts on the issue the previous commit created.
    trck_must(&s.work, &["set", "ccccccc", "--priority", "urgent"]);
    trck_must(&s.work, &["label", "ccccccc", "--add", "later"]);
    git_must(&s.work, &["remote", "set-url", "origin", &url]);

    remote_moves(&s, "fffffff");
    git_must(&s.work, &["fetch", "-q", "origin"]);
    trck_must(&s.work, &["new", "Trigger", "--id", "ggggggg", "--empty"]);

    let shown = trck_must(&s.work, &["show", "ccccccc"]);
    assert!(shown.contains("urgent"), "the set was dropped:\n{shown}");
    assert!(shown.contains("later"), "the label was dropped:\n{shown}");
}

/// Same pending commits, same remote tree, same result — twice over.
#[test]
fn replay_is_deterministic() {
    let mut indexes = Vec::new();
    for run in 0..2 {
        let Some(s) = Scenario::build(&format!("stack-determinism-{run}")) else {
            return;
        };
        stack_offline(&s.work, &["ccccccc", "ddddddd"]);
        remote_moves(&s, "fffffff");
        git_must(&s.work, &["fetch", "-q", "origin"]);
        trck_must(&s.work, &["new", "Trigger", "--id", "ggggggg", "--empty"]);
        indexes.push(s.show(LOCAL_REF, "index.jsonl").expect("an index"));
    }
    assert_eq!(indexes[0], indexes[1], "two identical runs produced different trackers");
}

/// An op that no longer applies stops the sequence, names itself, and leaves the ref alone.
#[test]
fn an_op_that_no_longer_applies_reports_and_changes_nothing() {
    let Some(s) = Scenario::build("stack-conflict") else {
        return;
    };
    // Filed offline with a forced id.
    let url = git_must(&s.work, &["remote", "get-url", "origin"]);
    git_must(&s.work, &["remote", "set-url", "origin", UNREACHABLE]);
    trck_must(&s.work, &["new", "Mine", "--id", "ccccccc", "--empty"]);
    git_must(&s.work, &["remote", "set-url", "origin", &url]);

    // Someone else files with the same id, and gets there first. `new` refuses an id that is
    // taken, so this pending op genuinely cannot be replayed — which is the point. The remote
    // moves *forward* from a commit this clone has, so what is pending is exactly what this
    // clone wrote, and no fixture commit is dragged into the stack.
    remote_moves(&s, "ccccccc");
    git_must(&s.work, &["fetch", "-q", "origin"]);
    assert!(
        !common::git_ok(&s.work, &["merge-base", "--is-ancestor", &format!("origin/{TRACKER_BRANCH}"), LOCAL_REF]),
        "the fixture is not diverged, so the push would fast-forward and never replay"
    );

    let before = sha(&s.work, LOCAL_REF);
    let out = trck(&s.work, &["new", "Trigger", "--id", "ggggggg", "--empty"]);
    // The write itself happened — its commit is on the branch, which is why the share is
    // reported rather than the verb failing outright. What must not have happened is the
    // rebuild leaving the branch part-way through a replay.
    let combined = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));

    assert!(!out.status.success(), "the replay had nothing to fail on — the push was accepted: {combined}");
    assert!(combined.contains("no longer applies"), "the refusal must name the failure: {combined}");
    assert!(combined.contains("ccccccc"), "and the operation that could not be replayed: {combined}");
    assert_eq!(git_must(&s.work, &["rev-parse", &format!("{LOCAL_REF}~1")]), before, "the branch is not the write's commit on top of what was there");
    assert_ne!(sha(&s.work, LOCAL_REF), sha(&s.work, &format!("origin/{TRACKER_BRANCH}")), "the branch was left on the remote's tip");
    let listed = trck_must(&s.work, &["list"]);
    assert!(listed.contains("ccccccc"), "the op that could not be replayed lost its issue:\n{listed}");
    assert!(listed.contains("ggggggg"), "the write that triggered the rebuild was lost:\n{listed}");
}

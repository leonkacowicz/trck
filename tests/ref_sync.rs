//! `trck sync`, and the pending-changes note that sends people to it.
//!
//! Two decisions elsewhere only work because this verb exists. Reads never fetch, so a read
//! on a plane answers instead of failing. Writes never fail on an unreachable remote, because
//! the commit is anchored on the local branch before the push is attempted. Both leave the
//! same residue — local commits the remote has not got — and without one verb that clears it,
//! that residue is silent, which makes it a data-loss story told slowly.

// Tests assert; that is their job. The crate denies unwrap/expect/panic because a malformed
// tracker must produce a diagnostic rather than a stack trace, but a test that cannot panic
// cannot fail.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::{Scenario, TRACKER_BRANCH, git_must, trck, trck_must};
use std::path::Path;

const UNREACHABLE: &str = "/trck-no-such-remote.git";

/// Run `args` with the remote unreachable, and put the remote back afterwards.
fn offline(work: &Path, args: &[&str]) -> std::process::Output {
    let url = git_must(work, &["remote", "get-url", "origin"]);
    git_must(work, &["remote", "set-url", "origin", UNREACHABLE]);
    let out = trck(work, args);
    git_must(work, &["remote", "set-url", "origin", &url]);
    out
}

fn sha(work: &Path, rev: &str) -> String {
    git_must(work, &["rev-parse", rev])
}

/// The note is the whole point: an unshared write that says nothing is indistinguishable
/// from a shared one until someone else cannot see the issue.
#[test]
fn a_write_that_could_not_push_says_what_is_pending() {
    let Some(s) = Scenario::build("sync-note") else {
        return;
    };
    let out = offline(&s.work, &["new", "Filed offline", "--id", "ccccccc", "--empty"]);
    assert!(out.status.success(), "an unshared write is not a failed write: {out:?}");
    let said = String::from_utf8_lossy(&out.stdout);
    assert!(said.contains("1 unpushed change"), "{said}");
    assert!(!said.contains("changes"), "one change is not plural: {said}");
    assert!(said.contains("trck sync"), "the note must name the remedy: {said}");
}

#[test]
fn the_count_grows_with_each_unshared_write() {
    let Some(s) = Scenario::build("sync-count") else {
        return;
    };
    offline(&s.work, &["new", "One", "--id", "ccccccc", "--empty"]);
    let out = offline(&s.work, &["new", "Two", "--id", "ddddddd", "--empty"]);
    let said = String::from_utf8_lossy(&out.stdout);
    assert!(said.contains("2 unpushed changes"), "{said}");
}

/// A write that *did* reach the remote says nothing extra — the note is a report of a
/// problem, and a clean write does not have one.
#[test]
fn a_write_that_pushed_says_nothing_about_pending() {
    let Some(s) = Scenario::build("sync-clean") else {
        return;
    };
    let out = trck_must(&s.work, &["new", "Filed online", "--id", "ccccccc", "--empty"]);
    assert!(!out.contains("unpushed"), "a shared write reported pending work: {out}");
}

#[test]
fn sync_pushes_what_is_waiting() {
    let Some(s) = Scenario::build("sync-push") else {
        return;
    };
    offline(&s.work, &["new", "Filed offline", "--id", "ccccccc", "--empty"]);
    let local = sha(&s.work, TRACKER_BRANCH);
    assert_ne!(local, sha(&s.work, &format!("origin/{TRACKER_BRANCH}")), "the fixture is not pending");

    let out = trck_must(&s.work, &["sync"]);
    assert!(out.contains("pushed 1 change"), "{out}");
    assert_eq!(sha(&s.work, &format!("origin/{TRACKER_BRANCH}")), local, "the remote did not take it");

    // And the note is gone, because the reason for it is.
    let after = trck_must(&s.work, &["new", "Filed online", "--id", "ddddddd", "--empty"]);
    assert!(!after.contains("unpushed"), "{after}");
}

#[test]
fn sync_with_nothing_to_do_says_so_and_succeeds() {
    let Some(s) = Scenario::build("sync-noop") else {
        return;
    };
    // A local branch equal to the remote: nothing waiting, nothing new.
    git_must(&s.work, &["update-ref", &format!("refs/heads/{TRACKER_BRANCH}"), &sha(&s.work, &format!("origin/{TRACKER_BRANCH}"))]);
    let out = trck_must(&s.work, &["sync"]);
    assert!(out.contains("already in sync"), "{out}");
}

/// Offline, `sync` reports the network and leaves the work where it is. The one thing it must
/// not do is lose the commit it could not push.
#[test]
fn sync_offline_reports_and_keeps_the_work() {
    let Some(s) = Scenario::build("sync-offline") else {
        return;
    };
    offline(&s.work, &["new", "Filed offline", "--id", "ccccccc", "--empty"]);
    let pending = sha(&s.work, TRACKER_BRANCH);

    let out = offline(&s.work, &["sync"]);
    assert!(!out.status.success(), "sync cannot succeed with no remote to reach");
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("not lost"), "the refusal must say the work survived: {err}");

    assert_eq!(sha(&s.work, TRACKER_BRANCH), pending, "the pending commit moved");
    let listed = trck_must(&s.work, &["list"]);
    assert!(listed.contains("ccccccc"), "the offline issue vanished:\n{listed}");
}

/// A directory tracker has no remote of its own, and saying "already in sync" would imply it
/// had one.
#[test]
fn sync_refuses_a_directory_backed_tracker() {
    let Some(s) = Scenario::build("sync-dir") else {
        return;
    };
    trck_must(&s.work, &["init", "issues"]);
    let out = trck(&s.work, &["sync"]);
    assert!(!out.status.success(), "a directory tracker has nothing to sync");
    assert!(String::from_utf8_lossy(&out.stderr).contains("directory"), "{:?}", out.stderr);
}

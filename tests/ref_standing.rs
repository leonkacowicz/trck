//! Which of the two conventional refs a read answers from.
//!
//! `origin/trck-issues` is not the answer on its own. A write that could not be pushed lives
//! on the local branch, and reading past it means filing an issue offline and then not seeing
//! it — the failure this whole rule exists to prevent.
//!
//! | local vs `origin/trck-issues` | read |
//! |---|---|
//! | ahead, or equal | local |
//! | behind | fast-forward local, read local |
//! | diverged | local, **and say so** |
//! | absent | `origin/trck-issues` |
//!
//! The fixture (`#32gyghs`) gives a clone with only the remote-tracking ref, so each case
//! here is built by putting a local branch somewhere and looking at what `list` answers.

// Tests assert; that is their job. The crate denies unwrap/expect/panic because a malformed
// tracker must produce a diagnostic rather than a stack trace, but a test that cannot panic
// cannot fail.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::{Scenario, TRACKER_BRANCH, git, git_must, trck, trck_must};
use std::path::Path;

/// The two seeded issues: the first commit holds `aaaaaaa`, the second adds `bbbbbbb`.
const FIRST: &str = "aaaaaaa";
const SECOND: &str = "bbbbbbb";

fn sha(work: &Path, rev: &str) -> String {
    git_must(work, &["rev-parse", rev])
}

/// Put the local branch at `rev` without checking anything out — the working tree is on an
/// unrelated branch and must stay there.
fn point_local_at(work: &Path, rev: &str) {
    let target = sha(work, rev);
    git_must(work, &["update-ref", &format!("refs/heads/{TRACKER_BRANCH}"), &target]);
}

/// File an issue with the remote out of reach — what "filed offline" actually takes.
///
/// A write pushes now, so a local branch only gets *ahead* of the remote when the push cannot
/// happen. The command fails, and should: the work did not reach the shared tracker and
/// reporting success would be a lie. What it leaves behind is the point — the commit is on the
/// local branch, which is the state every test below is about.
fn file_offline(work: &Path, args: &[&str]) {
    let url = git_must(work, &["remote", "get-url", "origin"]);
    git_must(work, &["remote", "set-url", "origin", "/trck-no-such-remote.git"]);
    let out = trck(work, args);
    assert!(!out.status.success(), "an unreachable remote must not report success: {out:?}");
    git_must(work, &["remote", "set-url", "origin", &url]);
}

#[test]
fn with_no_local_branch_the_remote_tracking_ref_answers() {
    let Some(s) = Scenario::build("standing-absent") else {
        return;
    };
    assert!(!common::git_ok(&s.work, &["rev-parse", "--verify", "--quiet", &format!("refs/heads/{TRACKER_BRANCH}")]), "the fixture already has a local branch");
    let out = trck_must(&s.work, &["list"]);
    assert!(out.contains(FIRST) && out.contains(SECOND), "{out}");
}

#[test]
fn an_equal_local_branch_answers_and_nothing_moves() {
    let Some(s) = Scenario::build("standing-equal") else {
        return;
    };
    point_local_at(&s.work, &format!("origin/{TRACKER_BRANCH}"));
    let before = sha(&s.work, TRACKER_BRANCH);

    let out = trck_must(&s.work, &["list"]);
    assert!(out.contains(FIRST) && out.contains(SECOND), "{out}");
    assert_eq!(sha(&s.work, TRACKER_BRANCH), before, "a read moved the ref");
}

/// The case the rule exists for: a write that could not be pushed is still the truth.
#[test]
fn a_local_branch_that_is_ahead_answers() {
    let Some(s) = Scenario::build("standing-ahead") else {
        return;
    };
    point_local_at(&s.work, &format!("origin/{TRACKER_BRANCH}"));
    // A write to the local ref — exactly what an unpushed `trck new` leaves behind.
    file_offline(&s.work, &["--ref", TRACKER_BRANCH, "new", "Filed offline", "--id", "ccccccc", "--empty"]);
    let ahead = sha(&s.work, TRACKER_BRANCH);
    assert_ne!(ahead, sha(&s.work, &format!("origin/{TRACKER_BRANCH}")), "the write did not advance the local ref");

    let out = trck_must(&s.work, &["list"]);
    assert!(out.contains("ccccccc"), "the unpushed issue is invisible:\n{out}");
    assert_eq!(sha(&s.work, TRACKER_BRANCH), ahead, "a read moved the ref");
}

/// Behind: fast-forward, then read. Asserted on the ref as well as the output, because
/// reading the remote without moving the branch would look identical here.
#[test]
fn a_local_branch_that_is_behind_is_fast_forwarded() {
    let Some(s) = Scenario::build("standing-behind") else {
        return;
    };
    point_local_at(&s.work, &format!("origin/{TRACKER_BRANCH}~1"));
    let remote = sha(&s.work, &format!("origin/{TRACKER_BRANCH}"));
    assert_ne!(sha(&s.work, TRACKER_BRANCH), remote, "the fixture is not behind");

    let out = trck_must(&s.work, &["list"]);
    assert!(out.contains(SECOND), "the newer issue is missing, so it did not fast-forward:\n{out}");
    assert_eq!(sha(&s.work, TRACKER_BRANCH), remote, "the branch was not fast-forwarded");
}

/// Diverged: local wins because it holds work that exists nowhere else, and the reader is
/// told, because the alternative is a listing quietly missing what landed remotely.
#[test]
fn a_diverged_local_branch_answers_and_says_so() {
    let Some(s) = Scenario::build("standing-diverged") else {
        return;
    };
    point_local_at(&s.work, &format!("origin/{TRACKER_BRANCH}~1"));
    file_offline(&s.work, &["--ref", TRACKER_BRANCH, "new", "Filed offline", "--id", "ccccccc", "--empty"]);
    let local = sha(&s.work, TRACKER_BRANCH);

    let out = trck(&s.work, &["list"]);
    assert!(out.status.success(), "a diverged tracker is readable, not an error");
    let listed = String::from_utf8_lossy(&out.stdout);
    assert!(listed.contains("ccccccc"), "read the remote instead of the local work:\n{listed}");
    assert!(!listed.contains(SECOND), "the remote's newer issue cannot be there — the refs diverged:\n{listed}");

    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("diverged"), "no warning: {err}");
    assert!(err.contains("trck sync"), "the warning must name the remedy: {err}");
    // The warning is on stderr precisely so this holds.
    assert!(!listed.contains("diverged"), "the warning leaked into stdout:\n{listed}");

    assert_eq!(sha(&s.work, TRACKER_BRANCH), local, "a read moved a diverged ref");
}

/// The rule that makes the fast-forward safe to do from a *read*: it is the only move, and
/// it never discards anything.
#[test]
fn a_read_never_moves_the_local_ref_except_by_fast_forward() {
    let Some(s) = Scenario::build("standing-noclobber") else {
        return;
    };
    point_local_at(&s.work, &format!("origin/{TRACKER_BRANCH}~1"));
    let before = sha(&s.work, TRACKER_BRANCH);
    trck_must(&s.work, &["list"]);
    let after = sha(&s.work, TRACKER_BRANCH);

    // It moved — and only forwards: what it held before is still reachable from where it is.
    assert_ne!(after, before);
    assert!(common::git_ok(&s.work, &["merge-base", "--is-ancestor", &before, &after]), "the ref moved somewhere that does not contain what it held");
}

/// Nothing above reaches the network. The remote is unreachable throughout, which is also
/// the state someone offline is actually in.
#[test]
fn none_of_it_fetches() {
    let Some(s) = Scenario::build("standing-nofetch") else {
        return;
    };
    point_local_at(&s.work, &format!("origin/{TRACKER_BRANCH}~1"));
    git_must(&s.work, &["remote", "set-url", "origin", "/nonexistent/there-is-no-remote-here.git"]);

    let out = trck_must(&s.work, &["list"]);
    assert!(out.contains(SECOND), "the fast-forward needed the network:\n{out}");
    assert_eq!(git(&s.work, &["rev-parse", TRACKER_BRANCH]), git(&s.work, &["rev-parse", &format!("origin/{TRACKER_BRANCH}")]));
}

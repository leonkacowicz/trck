//! What `trck serve` does to the tracker ref while nobody is looking.
//!
//! Everything here needs two things no unit test has: a real remote that can move
//! independently, and a process that is still running while the assertion is made. The whole
//! feature is about the second — a page rendered from a ref that somebody else advanced — so
//! a test that started the server, stopped it, and then looked would be testing the ordinary
//! read path with extra steps.
//!
//! **Nothing here sleeps for a fixed time.** The timer being waited on lives in another
//! process, so every wait is a poll with a generous deadline: on a loaded runner a fixed
//! sleep is a flake, and on an idle one it is wasted wall clock.

// Tests assert; that is their job. The crate denies unwrap/expect/panic because a malformed
// tracker must produce a diagnostic rather than a stack trace, but a test that cannot panic
// cannot fail.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::{Scenario, Server, TRACKER_BRANCH, clone_of, git_must, trck_must};
use std::path::Path;

/// How long a waiting assertion gives up after. Long next to a one-second poll interval, and
/// deliberately so: what is being waited for is another process's timer plus a `git fetch`
/// over a local path, and the fast path does not pay for the headroom.
const DEADLINE: u64 = 30;

/// A URL that cannot be a repository, for taking the remote away without touching the clone.
const UNREACHABLE: &str = "/trck-no-such-remote.git";

fn sha(dir: &Path, rev: &str) -> String {
    git_must(dir, &["rev-parse", rev])
}

/// Give the clone a local tracker branch at the remote's tip.
///
/// A *fresh* clone has only `origin/trck-issues` — the local branch appears the first time
/// this clone writes — and both shapes have to keep a page current, by different routes: with
/// a local branch the timer fast-forwards it, without one the served ref *is* the
/// remote-tracking one and the fetch moves it directly. The fixture is fresh, so a test about
/// the first shape has to ask for it.
fn with_local_branch(work: &Path) {
    git_must(work, &["branch", TRACKER_BRANCH, &format!("origin/{TRACKER_BRANCH}")]);
}

/// **The feature.** Somebody else pushes; the served page catches up without anyone touching
/// this process, and the local branch is genuinely moved rather than the page quietly reading
/// the remote-tracking ref instead.
#[test]
fn a_push_from_elsewhere_reaches_an_already_running_page() {
    let Some(s) = Scenario::build("serve-poll") else {
        return;
    };
    with_local_branch(&s.work);
    let server = Server::start(&s.work, &["--poll", "1"]);
    let before = server.get("/");
    assert!(before.contains("Seeded issue"), "the server is not serving the fixture tracker");
    assert!(!before.contains("Landed elsewhere"), "the fixture already holds the issue this test is about to file");
    let was = sha(&s.work, TRACKER_BRANCH);

    // A second clone, because a write through `s.work` would move that clone's own branch —
    // which is the case where there is nothing to discover. The point is a remote that moved
    // under a process that had no way of knowing.
    let elsewhere = clone_of(s.work.parent().expect("a parent"), &s.origin, "elsewhere", &[]);
    trck_must(&elsewhere, &["new", "Landed elsewhere", "--id", "ddddddd", "--empty"]);

    assert!(server.wait_for(DEADLINE, |s| s.get("/").contains("Landed elsewhere")), "the page never picked up the push:\n{}", server.log());

    // Not merely visible: the local branch was fast-forwarded onto it. Reading it out of
    // `origin/trck-issues` instead would look identical from the browser and would leave the
    // clone behind for every other verb.
    let now = sha(&s.work, TRACKER_BRANCH);
    assert_ne!(now, was, "the page updated without the local branch moving");
    assert_eq!(now, sha(&s.work, &format!("origin/{TRACKER_BRANCH}")), "the local branch is not on the remote's tip");

    // And it said so, once, on stderr — the running log is how an operator sees any of this.
    let log = server.log();
    assert!(log.contains(&now[..8]), "the log does not name the commit it moved to:\n{log}");
    assert!(log.contains(TRACKER_BRANCH), "the log does not name the branch:\n{log}");
}

/// The other shape: a clone that has never written, so it has no local tracker branch and the
/// ref being served *is* `origin/trck-issues`. Nothing is fast-forwarded — the fetch moves the
/// served ref directly — and the page has to end up just as current, because "which of the two
/// refs this clone happens to have" is not something anyone reading a page knows or should.
#[test]
fn a_clone_with_only_the_remote_tracking_ref_is_kept_current_too() {
    let Some(s) = Scenario::build("serve-fresh") else {
        return;
    };
    assert!(!common::git_ok(&s.work, &["rev-parse", "--verify", TRACKER_BRANCH]), "the fixture clone was supposed to be fresh");
    let server = Server::start(&s.work, &["--poll", "1"]);
    assert!(server.get("/").contains("Seeded issue"), "the server is not serving the fixture tracker");

    let elsewhere = clone_of(s.work.parent().expect("a parent"), &s.origin, "elsewhere", &[]);
    trck_must(&elsewhere, &["new", "Landed elsewhere", "--id", "ddddddd", "--empty"]);

    assert!(server.wait_for(DEADLINE, |s| s.get("/").contains("Landed elsewhere")), "the page never picked up the push:\n{}", server.log());
    // Still fresh: the timer moved the remote-tracking ref and never invented a local branch.
    assert!(!common::git_ok(&s.work, &["rev-parse", "--verify", TRACKER_BRANCH]), "serve created a local branch this clone never asked for");
}

/// A tab left open on a laptop that went offline. The remote is gone, the process is not: it
/// keeps serving the local ref, and it says why rather than leaving a page that silently stops
/// being current.
#[test]
fn an_unreachable_remote_is_reported_and_the_local_ref_is_still_served() {
    let Some(s) = Scenario::build("serve-offline") else {
        return;
    };
    git_must(&s.work, &["remote", "set-url", "origin", UNREACHABLE]);
    let server = Server::start(&s.work, &["--poll", "1"]);

    assert!(server.wait_for(DEADLINE, |s| s.log().contains("cannot reach origin")), "an unreachable remote said nothing:\n{}", server.log());
    assert!(server.log().contains("serving the local"), "the warning does not say what it is doing instead:\n{}", server.log());

    // The process is the thing under test here: a fetch that fails must not take it down.
    let page = server.get("/");
    assert!(page.starts_with("HTTP/1.1 200 OK\r\n"), "the server stopped serving when the remote went away");
    assert!(page.contains("Seeded issue"), "the local ref is no longer being served");

    // Once, not once per interval. The interval is a second and the wait above took at least
    // one, so a loop that logged every tick would have said it several times by now.
    assert_eq!(server.log().matches("cannot reach origin").count(), 1, "the offline warning repeats every tick:\n{}", server.log());

    // And it says so when the remote comes back, because otherwise the last thing anybody
    // watching ever read was that it was gone.
    git_must(&s.work, &["remote", "set-url", "origin", &s.origin.display().to_string()]);
    assert!(server.wait_for(DEADLINE, |s| s.log().contains("reachable again")), "the recovery went unreported:\n{}", server.log());
}

/// Diverged is the one state that is never resolved automatically: local holds work that
/// exists nowhere else, and the remote holds work this clone has not got. Both are real, and
/// picking one would lose the other.
#[test]
fn a_diverged_pair_is_reported_and_never_resolved() {
    let Some(s) = Scenario::build("serve-diverged") else {
        return;
    };
    // Local work that cannot be pushed, so the branch moves here and nowhere else.
    git_must(&s.work, &["remote", "set-url", "origin", UNREACHABLE]);
    trck_must(&s.work, &["new", "Filed offline", "--id", "eeeeeee", "--empty"]);
    let local = sha(&s.work, TRACKER_BRANCH);

    // And somebody else's work on the remote, so neither side is an ancestor of the other.
    let elsewhere = clone_of(s.work.parent().expect("a parent"), &s.origin, "elsewhere", &[]);
    trck_must(&elsewhere, &["new", "Landed elsewhere", "--id", "fffffff", "--empty"]);

    git_must(&s.work, &["remote", "set-url", "origin", &s.origin.display().to_string()]);
    let server = Server::start(&s.work, &["--poll", "1"]);

    assert!(server.wait_for(DEADLINE, |s| s.log().contains("diverged")), "divergence went unreported:\n{}", server.log());
    assert!(server.log().contains("trck sync"), "the warning does not name the remedy:\n{}", server.log());
    assert_eq!(server.log().matches("diverged").count(), 1, "the divergence warning repeats every tick:\n{}", server.log());

    // Nothing moved. That is the criterion: a diverged pair is surfaced, not reconciled.
    assert_eq!(sha(&s.work, TRACKER_BRANCH), local, "serve resolved a divergence by moving the local branch");
    // And the page shows the local side, which is the side holding work that exists nowhere
    // else — the same choice every read verb makes.
    let page = server.get("/");
    assert!(page.contains("Filed offline"), "the page dropped the unshared local work:\n{}", server.log());
}

/// Off means off. A clone with no remote has nothing for a timer to discover, and `--poll 0`
/// is how somebody says so for one that has.
#[test]
fn polling_can_be_turned_off_and_says_nothing_when_there_is_nothing_to_poll() {
    let Some(s) = Scenario::build("serve-nopoll") else {
        return;
    };
    let elsewhere = clone_of(s.work.parent().expect("a parent"), &s.origin, "elsewhere", &[]);
    with_local_branch(&s.work);

    let off = Server::start(&s.work, &["--poll", "0"]);
    let was = sha(&s.work, TRACKER_BRANCH);
    trck_must(&elsewhere, &["new", "Landed elsewhere", "--id", "ddddddd", "--empty"]);
    // A negative with a deadline is only honest because the answer can never change: with the
    // timer off, nothing in this process will ever look at the remote.
    assert!(!off.wait_for(3, |s| s.get("/").contains("Landed elsewhere")), "the page updated with polling off");
    assert_eq!(sha(&s.work, TRACKER_BRANCH), was, "the branch moved with polling off");
    assert_eq!(off.log(), "", "polling is off, so there is nothing to say");
    drop(off);

    // No remote at all: the timer is not started, and the reason is stated once rather than
    // rediscovered every interval.
    git_must(&s.work, &["remote", "remove", "origin"]);
    let lonely = Server::start(&s.work, &["--poll", "1"]);
    assert!(lonely.wait_for(DEADLINE, |s| s.log().contains("not polling")), "a remoteless clone said nothing:\n{}", lonely.log());
    assert!(lonely.get("/").contains("Seeded issue"), "a remoteless clone is still a tracker worth serving");
}

/// The other half of "no parsed index is held across requests": a write through *this* clone
/// moves the local ref with no fetch involved at all, and the very next request must show it.
/// Every other verb resolves a tracker and exits, so this is the only place the bug can live.
#[test]
fn a_local_write_is_visible_to_the_next_request() {
    let Some(s) = Scenario::build("serve-local-write") else {
        return;
    };
    let server = Server::start(&s.work, &["--poll", "0"]);
    assert!(!server.get("/").contains("Filed while serving"), "the fixture already holds this issue");

    trck_must(&s.work, &["new", "Filed while serving", "--id", "ggggggg", "--empty"]);

    // No wait: the ref moved before `trck new` returned, and the page is rendered from
    // whatever it points at when the request arrives.
    assert!(server.get("/").contains("Filed while serving"), "the served page is a snapshot from startup");
}

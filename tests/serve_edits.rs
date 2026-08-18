//! `POST /edits` against a real tracker on a real ref.
//!
//! The unit tests beside `src/serve/` map a field to an operation and check the shape of the
//! answer; none of them writes anything. What is only true of the whole path is here: an edit
//! posted over a socket becomes a commit on the tracker branch, pushed to a remote, through the
//! same verb functions the CLI calls — and a refusal comes back as the engine's own words with
//! the tracker untouched.
//!
//! The rejected-push case is the one that needs all of it at once: a remote that moved under
//! this process, a write that cannot fast-forward onto it, and a rebuild that has to land
//! anyway. Nothing short of two clones and a bare origin can stage that.

// Tests assert; that is their job. The crate denies unwrap/expect/panic because a malformed
// tracker must produce a diagnostic rather than a stack trace, but a test that cannot panic
// cannot fail.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::{Scenario, Server, TRACKER_BRANCH, clone_of, git_must, trck_must};
use std::path::Path;

/// Post a batch and hand back the response, head and all.
fn post(server: &Server, body: &str) -> String {
    server.request(&format!(
        "POST /edits HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        server.port,
        body.len()
    ))
}

/// One edit, as the page stages it.
fn edit(id: &str, field: &str, value: &str) -> String {
    format!(r#"{{"edits": [{{"id": "{id}", "field": "{field}", "value": "{value}"}}]}}"#)
}

fn sha(dir: &Path, rev: &str) -> String {
    git_must(dir, &["rev-parse", rev])
}

/// A server over the fixture clone, with the local tracker branch in place and polling off —
/// what moves the ref in these tests is the write under test, not a timer.
fn serving(s: &Scenario) -> Server {
    git_must(&s.work, &["branch", TRACKER_BRANCH, &format!("origin/{TRACKER_BRANCH}")]);
    Server::start(&s.work, &["--poll", "0"])
}

/// **The feature.** A staged status change posted from the page becomes a commit on the
/// tracker branch and a push, with no subprocess anywhere in between.
#[test]
fn a_posted_status_change_commits_and_pushes() {
    let Some(s) = Scenario::build("serve-edit-mv") else {
        return;
    };
    let server = serving(&s);
    let was = sha(&s.work, TRACKER_BRANCH);

    let res = post(&server, &edit("aaaaaaa", "status", "in-progress"));
    assert!(res.starts_with("HTTP/1.1 200 OK\r\n"), "{res}");
    assert!(res.contains("application/json"), "the answer is not JSON: {res}");
    assert!(res.contains("\"ok\": true"), "{res}");

    // The commit is real, and it is on the branch — not merely in the answer.
    let now = sha(&s.work, TRACKER_BRANCH);
    assert_ne!(now, was, "the write did not move the tracker branch");
    // And the response says where the ref is, which is how a page tells its own write from
    // somebody else's.
    assert!(res.contains(&now), "the answer does not carry the new sha:\n{res}");
    // Pushed, not merely committed: the remote has it, and the answer says nothing is pending.
    assert_eq!(sha(&s.work, &format!("origin/{TRACKER_BRANCH}")), now, "the write was not pushed");
    assert!(res.contains("\"pending\": 0"), "{res}");

    // The tracker itself agrees, read back through the CLI rather than out of the answer.
    assert!(trck_must(&s.work, &["show", "aaaaaaa"]).contains("in-progress"), "the issue did not move");
    // And the page rendered from the same process shows it, with no restart.
    assert!(server.get("/").contains("in-progress"), "the served page is a snapshot from startup");
}

/// The other two verbs an edit can mean. `set` for a scalar field, `dep` for an edge — and the
/// edge is posted as the whole desired list, which is how a control that edits a list works.
#[test]
fn a_set_and_a_dep_go_through_the_same_path() {
    let Some(s) = Scenario::build("serve-edit-set-dep") else {
        return;
    };
    let server = serving(&s);

    let res = post(&server, &edit("aaaaaaa", "priority", "high"));
    assert!(res.contains("\"ok\": true"), "{res}");
    assert!(trck_must(&s.work, &["show", "aaaaaaa"]).contains("high"), "the priority did not change");

    // `bbbbbbb` is the fixture's second issue. Posting the desired list, not a delta.
    let res = post(&server, r#"{"edits": [{"id": "aaaaaaa", "field": "requires", "value": ["bbbbbbb"]}]}"#);
    assert!(res.contains("\"ok\": true"), "{res}");
    let shown = trck_must(&s.work, &["show", "aaaaaaa"]);
    assert!(shown.contains("bbbbbbb"), "the dependency was not added:\n{shown}");

    // And removing it again is the same shape with the id left out of the list.
    let res = post(&server, r#"{"edits": [{"id": "aaaaaaa", "field": "requires", "value": []}]}"#);
    assert!(res.contains("\"ok\": true"), "{res}");
    assert!(!trck_must(&s.work, &["show", "aaaaaaa"]).contains("depends_on"), "the dependency was not removed");
}

/// A refusal is the engine's own diagnostic, and the tracker is exactly where it was. The
/// wording is not reworded here on purpose: the same failure must not have one vocabulary in
/// the terminal and another in the browser.
#[test]
fn a_validation_failure_changes_nothing_and_says_what_the_cli_would_say() {
    let Some(s) = Scenario::build("serve-edit-refuse") else {
        return;
    };
    let server = serving(&s);
    let was = sha(&s.work, TRACKER_BRANCH);

    let res = post(&server, &edit("aaaaaaa", "priority", "nosuch"));
    assert!(res.starts_with("HTTP/1.1 422 "), "a refusal from the tracker is a 422: {res}");
    assert!(res.contains("\"ok\": false"), "{res}");
    assert_eq!(sha(&s.work, TRACKER_BRANCH), was, "a refused write moved the branch");

    // The same words the CLI produces for the same mistake, character for character.
    let cli =
        String::from_utf8_lossy(&common::trck(&s.work, &["set", "aaaaaaa", "--priority", "nosuch"]).stderr).trim().trim_start_matches("error: ").to_string();
    assert!(!cli.is_empty(), "the CLI refused silently, so there is nothing to compare");
    assert!(res.contains(&cli), "the page is told something other than what the CLI says:\n{res}\nCLI: {cli}");
}

/// A batch stops at the first refusal, and what already landed stays landed — each operation
/// is its own commit. The answer has to say both, because "it failed" and "nothing happened"
/// are different sentences and only one of them is true here.
#[test]
fn a_batch_stops_at_the_first_refusal_and_reports_what_landed() {
    let Some(s) = Scenario::build("serve-edit-batch") else {
        return;
    };
    let server = serving(&s);

    let body = r#"{"edits": [
        {"id": "aaaaaaa", "field": "status", "value": "in-progress"},
        {"id": "aaaaaaa", "field": "priority", "value": "nosuch"},
        {"id": "bbbbbbb", "field": "status", "value": "in-progress"}
    ]}"#;
    let res = post(&server, body);
    assert!(res.starts_with("HTTP/1.1 422 "), "{res}");
    // The first landed.
    assert!(trck_must(&s.work, &["show", "aaaaaaa"]).contains("in-progress"), "the first edit did not land");
    // The third never ran, because the second stopped the batch.
    assert!(!trck_must(&s.work, &["show", "bbbbbbb"]).contains("in-progress"), "the batch kept going past a refusal");
    // And the answer says so rather than reading as though nothing happened. What it reports
    // is the verb's own words — for `mv` that is the body it wrote, which is what the CLI
    // prints too; the page counts these rather than reading them.
    assert!(res.contains("items/aaaaaaa"), "the answer does not report what landed:\n{res}");
    assert!(res.contains("bad priority 'nosuch'"), "the answer does not carry the engine's diagnostic:\n{res}");
}

/// **A rejected push is not a failure.** Somebody else's write landed between this one being
/// built and being pushed, so the operation is replayed on top of theirs and pushed again. From
/// the page that is an ordinary success — and both writes have to survive it.
#[test]
fn a_push_rejected_by_another_writer_is_replayed_rather_than_lost() {
    let Some(s) = Scenario::build("serve-edit-race") else {
        return;
    };
    let server = serving(&s);

    // Another clone lands a write the serving clone has never seen. Its branch is now behind,
    // and the write below is built on a base the remote has moved past.
    let elsewhere = clone_of(s.work.parent().expect("a parent"), &s.origin, "elsewhere", &[]);
    trck_must(&elsewhere, &["new", "Landed elsewhere", "--id", "ddddddd", "--empty"]);

    let res = post(&server, &edit("aaaaaaa", "status", "done"));
    assert!(res.contains("\"ok\": true"), "a rejected push is not a failed write:\n{res}");
    assert!(res.contains("\"pending\": 0"), "the write did not reach the remote:\n{res}");

    // Both writes are on the remote: the other clone's issue and this one's status change.
    git_must(&s.work, &["fetch", "-q", "origin", TRACKER_BRANCH]);
    let listed = trck_must(&s.work, &["list", "--all", "--flat"]);
    assert!(listed.contains("Landed elsewhere"), "the other writer's issue was lost:\n{listed}");
    assert!(trck_must(&s.work, &["show", "aaaaaaa"]).contains("done"), "this write was lost");
}

/// A request that is not a request is a 400 — the shape was wrong — while a tracker that says
/// no is a 422. Two different failures, and a page that treated them alike would retry the one
/// that can never succeed.
#[test]
fn a_malformed_request_is_told_apart_from_a_refused_one() {
    let Some(s) = Scenario::build("serve-edit-malformed") else {
        return;
    };
    let server = serving(&s);

    for bad in ["", "not json", "{}", r#"{"edits": [{"id": "aaaaaaa"}]}"#] {
        let res = post(&server, bad);
        assert!(res.starts_with("HTTP/1.1 400 "), "{bad:?} should be a 400:\n{res}");
    }
    // A field the endpoint does not offer is refused by name — the request never names a verb,
    // so there is nothing a client can ask for that was not intended.
    let res = post(&server, &edit("aaaaaaa", "resolution", "fixed"));
    assert!(res.starts_with("HTTP/1.1 422 "), "{res}");
    assert!(res.contains("not an editable field"), "{res}");

    // And the route takes POST alone, saying so the way a 405 is required to.
    let res = server.get("/edits");
    assert!(res.starts_with("HTTP/1.1 405 "), "{res}");
    assert!(res.contains("Allow: POST"), "{res}");
}

/// The page knows whether it has a process behind it, because the engine tells it. A written
/// file says `live: false` however it is later served, so it goes on offering commands to
/// paste rather than an Apply button that would post into nothing.
#[test]
fn a_served_page_is_live_and_a_written_one_is_not() {
    let Some(s) = Scenario::build("serve-edit-live") else {
        return;
    };
    let server = serving(&s);
    assert!(server.get("/").contains("\"live\": true"), "a served page must know it can write");

    trck_must(&s.work, &["html", "--out", "page.html"]);
    let written = std::fs::read_to_string(s.work.join("page.html")).expect("the written page");
    assert!(written.contains("\"live\": false"), "a written file has nothing to post to");
}

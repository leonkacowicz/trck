//! Reading a tracker out of a git ref.
//!
//! The claim is narrow and worth stating exactly: from a checkout with no `issues/`
//! directory, sitting on an unrelated branch with uncommitted edits, every read verb
//! answers from the tracker branch — with no checkout, no worktree and no network.
//!
//! These run through the `#32gyghs` harness, which builds that shape. What they add is the
//! part a fixture cannot assert on its own: that the verbs actually read it, that a broken
//! tracker reports the same inconsistency it would on disk, and that nothing reaches for
//! the network to do it.

// Tests assert; that is their job. The crate denies unwrap/expect/panic because a malformed
// tracker must produce a diagnostic rather than a stack trace, but a test that cannot panic
// cannot fail.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::{HOLED_BRANCH, SEEDED_BODY, Scenario, TRACKER_BRANCH, git_must, trck, trck_must};

/// The headline: nine verbs, one fixture, no directory.
#[test]
fn every_read_verb_answers_from_the_ref() {
    let Some(s) = Scenario::build("reads-all") else {
        return;
    };
    for verb in [vec!["list"], vec!["tree"], vec!["ready"], vec!["next"], vec!["deps", "aaaaaaa"], vec!["show", "aaaaaaa"], vec!["check"], vec!["summary"]] {
        let out = trck(&s.work, &verb);
        assert!(out.status.success(), "trck {verb:?}: {}", String::from_utf8_lossy(&out.stderr));
    }
    // `html` writes a file, so it gets an explicit destination rather than defaulting to
    // one inside a tracker directory that does not exist.
    let page = s.work.join("issues.html");
    trck_must(&s.work, &["html", "--out", &page.display().to_string()]);
    let html = std::fs::read_to_string(&page).expect("page");
    assert!(html.contains("Seeded issue"), "the page did not come from the ref");
}

#[test]
fn list_shows_the_issues_the_ref_holds() {
    let Some(s) = Scenario::build("reads-list") else {
        return;
    };
    let out = trck_must(&s.work, &["list"]);
    for id in ["aaaaaaa", "bbbbbbb"] {
        assert!(out.contains(id), "{id} missing:\n{out}");
    }
}

/// Bodies come out of the ref too — `show` is the only read verb that opens one.
#[test]
fn show_prints_a_body_read_from_the_ref() {
    let Some(s) = Scenario::build("reads-show") else {
        return;
    };
    let out = trck_must(&s.work, &["show", "aaaaaaa"]);
    assert!(out.contains("Seeded issue"), "the row is missing:\n{out}");
    assert!(out.contains(SEEDED_BODY), "the body did not come out of the ref:\n{out}");
}

/// The same inconsistency, reported the same way, whichever source it came from. A
/// different wording here would mean two diagnostics for one broken tracker.
#[test]
fn a_body_missing_from_the_ref_reads_like_a_missing_file() {
    let Some(s) = Scenario::build("reads-holed") else {
        return;
    };
    let out = trck(&s.work, &["--ref", &format!("origin/{HOLED_BRANCH}"), "show", "bbbbbbb"]);
    assert!(!out.status.success(), "a body that is not there was somehow shown");
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("file missing for #bbbbbbb"), "unexpected wording: {err}");
}

/// A revision that resolves but holds no tracker is a different mistake from one that does
/// not resolve, and it has to name the ref either way — otherwise the message is "no
/// tracker found here", which is false and sends you looking at your working directory.
#[test]
fn a_ref_that_is_not_a_tracker_names_the_ref() {
    let Some(s) = Scenario::build("reads-nontracker") else {
        return;
    };
    let out = trck(&s.work, &["--ref", "origin/main", "list"]);
    assert!(!out.status.success(), "the code branch was accepted as a tracker");
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("origin/main"), "the refusal must name the ref: {err}");
}

/// No read verb may reach the network.
///
/// Asserted by taking the network away: the remote is repointed at a path that does not
/// exist, so any `fetch` under a read would fail loudly. The remote-tracking ref is already
/// in the clone, so a verb that only reads keeps working.
#[test]
fn a_read_does_not_fetch() {
    let Some(s) = Scenario::build("reads-nofetch") else {
        return;
    };
    git_must(&s.work, &["remote", "set-url", "origin", "/nonexistent/there-is-no-remote-here.git"]);
    let out = trck_must(&s.work, &["list"]);
    assert!(out.contains("aaaaaaa"), "read failed with the remote unreachable:\n{out}");
}

/// `path` answers with somewhere to open. A ref-backed tracker has nowhere, and a path that
/// is not there — which is what joining a tracker directory that does not exist would
/// produce — is worse than being told so.
#[test]
fn path_is_honest_about_a_ref_backed_tracker() {
    let Some(s) = Scenario::build("reads-path") else {
        return;
    };
    let out = trck(&s.work, &["path", "aaaaaaa"]);
    let text = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    assert!(text.contains(TRACKER_BRANCH), "said nothing about where the tracker actually is: {text}");
    for line in text.lines().map(str::trim).filter(|l| l.starts_with('/')) {
        assert!(std::path::Path::new(line).exists(), "printed a path that does not exist: {line}");
    }
}

/// The rollup is the one output that only `summary` produces, so a ref-backed tracker gets
/// it on stdout rather than a refusal about having nowhere to write it.
#[test]
fn summary_prints_the_rollup_when_there_is_nowhere_to_write_it() {
    let Some(s) = Scenario::build("reads-summary") else {
        return;
    };
    let out = trck_must(&s.work, &["summary"]);
    assert!(out.contains("Seeded issue"), "the rollup did not come from the ref:\n{out}");
    assert!(!out.starts_with("wrote "), "claimed to write a file for a tracker with no directory:\n{out}");
}

/// `edit` against a ref-backed tracker, end to end: the body is read out of the ref, the
/// new one is committed back to it, and the next read sees it.
///
/// Nothing in `edit` knows which kind of tracker it has — it reads through the content
/// accessors and writes through a changeset — so this is the assertion that the two halves
/// actually meet.
#[test]
fn edit_reads_from_the_ref_and_writes_back_to_it() {
    let Some(s) = Scenario::build("reads-edit") else {
        return;
    };
    let out = trck_must(&s.work, &["edit", "aaaaaaa", "--body", "revised prose."]);
    assert!(out.contains("edited"), "{out}");

    let shown = trck_must(&s.work, &["show", "aaaaaaa"]);
    assert!(shown.contains("revised prose."), "the edit did not reach the ref:\n{shown}");
    assert!(!shown.contains(SEEDED_BODY), "the old body is still there:\n{shown}");

    // Nothing was written to the checkout: the tracker is still only on the branch.
    assert!(!s.work.join("issues").exists(), "a tracker directory appeared in the working tree");
}

/// The no-op rule holds for a ref-backed tracker too — and it matters more there, since an
/// empty commit on a shared branch is something everyone else has to rebase over.
#[test]
fn an_edit_that_changes_nothing_commits_nothing_to_the_ref() {
    let Some(s) = Scenario::build("reads-edit-noop") else {
        return;
    };
    let before = git_must(&s.work, &["rev-parse", &format!("origin/{TRACKER_BRANCH}")]);
    let out = trck_must(&s.work, &["edit", "aaaaaaa", "--body", SEEDED_BODY]);
    assert!(out.contains("unchanged"), "{out}");
    assert_eq!(git_must(&s.work, &["rev-parse", &format!("origin/{TRACKER_BRANCH}")]), before, "the ref moved for a no-op");
}

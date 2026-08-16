//! A clone that cannot see the tracker branch, and what it is told.
//!
//! `--single-branch` and `--depth` narrow `remote.origin.fetch` to one branch, which is what
//! `actions/checkout` does by default. `origin/trck-issues` then does not exist and never
//! will, however many times you fetch — and the honest-looking answer, *no tracker found*, is
//! wrong about what is true and points at a remedy that would make things worse.
//!
//! These run against the `#32gyghs` fixture's origin, cloned narrowly on purpose.

// Tests assert; that is their job. The crate denies unwrap/expect/panic because a malformed
// tracker must produce a diagnostic rather than a stack trace, but a test that cannot panic
// cannot fail.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::{Scenario, TRACKER_BRANCH, git_must, trck, trck_must};
use std::path::{Path, PathBuf};

/// A clone of the fixture's origin that fetches `main` and nothing else.
fn narrow_clone(s: &Scenario, name: &str) -> PathBuf {
    let root = s.work.parent().expect("the scenario root");
    let at = common::clone_of(root, &s.origin, name, &["--single-branch", "--branch", "main"]);
    // A narrow clone has no `issues/` either — the flip is what makes this reachable, and a
    // fixture that still had one would be testing the staging rule instead.
    let _ = std::fs::remove_dir_all(at.join("issues"));
    at
}

fn refspecs(at: &Path) -> String {
    git_must(at, &["config", "--get-all", "remote.origin.fetch"])
}

#[test]
fn a_narrow_clone_really_cannot_see_the_branch() {
    let Some(s) = Scenario::build("refspec-premise") else {
        return;
    };
    let at = narrow_clone(&s, "narrow-premise");
    // The premise, asserted rather than assumed: fetching does not help.
    git_must(&at, &["fetch", "-q", "origin"]);
    assert!(!common::git_ok(&at, &["rev-parse", "--verify", "--quiet", &format!("origin/{TRACKER_BRANCH}")]), "the clone is not narrow");
    assert!(!refspecs(&at).contains(TRACKER_BRANCH), "{}", refspecs(&at));
}

/// The diagnostic: what is true, and what to do about it.
#[test]
fn the_refusal_says_the_branch_is_hidden_rather_than_missing() {
    let Some(s) = Scenario::build("refspec-hidden") else {
        return;
    };
    let at = narrow_clone(&s, "narrow-hidden");

    let out = trck(&at, &["list"]);
    assert!(!out.status.success(), "a clone that cannot see the tracker listed one");
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("does not fetch it"), "unexpected wording: {err}");
    assert!(err.contains("trck repo setup-git"), "the refusal must name the remedy: {err}");
    assert!(err.contains("+refs/heads/trck-issues:"), "and the refspec it would add: {err}");
    // The wording that sent the reader wrong.
    assert!(!err.contains("trck init"), "still telling them to make a tracker: {err}");
}

/// A repository with no tracker anywhere keeps the old wording. The new message is only
/// earned by a remote that actually has the branch.
#[test]
fn a_repository_with_no_tracker_still_reads_the_old_way() {
    let Some(s) = Scenario::build("refspec-none") else {
        return;
    };
    let bare = s.work.parent().expect("root").join("no-tracker");
    std::fs::create_dir_all(&bare).expect("mkdir");
    git_must(&bare, &["init", "-q", "-b", "main"]);

    let out = trck(&bare, &["list"]);
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("no tracker found"), "{err}");
    assert!(err.contains("trck init"), "{err}");
}

/// Offline the question cannot be asked, and a guess would be worse than the old answer.
#[test]
fn offline_it_degrades_to_the_old_wording_rather_than_a_network_error() {
    let Some(s) = Scenario::build("refspec-offline") else {
        return;
    };
    let at = narrow_clone(&s, "narrow-offline");
    git_must(&at, &["remote", "set-url", "origin", "/trck-no-such-remote.git"]);

    let out = trck(&at, &["list"]);
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("no tracker found"), "a network error leaked into the diagnostic: {err}");
    assert!(!err.contains("ls-remote"), "{err}");
}

/// `setup-git` is the per-clone configuration verb, and this is a per-clone problem.
#[test]
fn setup_git_widens_the_refspec_and_the_branch_then_arrives() {
    let Some(s) = Scenario::build("refspec-setup") else {
        return;
    };
    let at = narrow_clone(&s, "narrow-setup");

    let said = trck_must(&at, &["repo", "setup-git"]);
    assert!(said.contains("skipped .gitattributes"), "{said}");
    assert!(said.contains("remote.origin.fetch"), "{said}");
    assert!(refspecs(&at).contains(TRACKER_BRANCH), "the refspec was not widened: {}", refspecs(&at));
    assert!(git_must(&at, &["config", "--get", "merge.trck-index.driver"]).contains("merge-index"));
    assert!(git_must(&at, &["config", "--get", "merge.trck-summary.driver"]).contains("merge-summary"));

    git_must(&at, &["fetch", "-q", "origin"]);
    assert!(common::git_ok(&at, &["rev-parse", "--verify", "--quiet", &format!("origin/{TRACKER_BRANCH}")]), "the branch still did not arrive");
}

#[test]
fn setup_git_is_idempotent() {
    let Some(s) = Scenario::build("refspec-idempotent") else {
        return;
    };
    let at = narrow_clone(&s, "narrow-idempotent");
    trck_must(&at, &["init", "issues"]);

    trck_must(&at, &["repo", "setup-git"]);
    let once = refspecs(&at);
    let said = trck_must(&at, &["repo", "setup-git"]);
    assert_eq!(refspecs(&at), once, "a second run added the refspec again");
    assert!(said.contains("already fetches"), "{said}");
}

/// A default clone is already fine, and saying otherwise would be noise on every setup.
#[test]
fn setup_git_leaves_a_default_clone_alone() {
    let Some(s) = Scenario::build("refspec-default") else {
        return;
    };
    let before = refspecs(&s.work);
    let said = trck_must(&s.work, &["repo", "setup-git"]);
    assert_eq!(refspecs(&s.work), before, "a wildcard refspec was widened anyway");
    assert!(said.contains("skipped .gitattributes"), "{said}");
    assert!(said.contains("already fetches"), "{said}");
}

#[test]
fn install_hook_explains_why_a_ref_tracker_needs_no_hook() {
    let Some(s) = Scenario::build("ref-hook") else {
        return;
    };

    let out = trck(&s.work, &["repo", "install-hook"]);
    assert!(!out.status.success(), "installed a hook for a ref-backed tracker");
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("working-tree commits cannot change it"), "{err}");
    assert!(err.contains("nothing to guard"), "{err}");
    assert!(!err.contains("not a git repository"), "{err}");
}

#[test]
fn setup_git_keeps_an_explicit_non_tracker_ref_strict() {
    let Some(s) = Scenario::build("ref-setup-explicit") else {
        return;
    };

    let out = trck(&s.work, &["--ref", "main", "repo", "setup-git"]);
    assert!(!out.status.success(), "configured git for a ref that is not a tracker");
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("trck.json"), "{err}");
}

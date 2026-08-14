//! The ref-backed tracker harness, checked against itself.
//!
//! Every task in the reads and writes tranches of `#sqzr7nk` asserts through
//! [`common::Scenario`], so the fixture it builds is load-bearing before any of them
//! exist. A harness that quietly built the wrong shape — a tracker committed on `main`
//! after all, a clone that never fetched the branch, a working tree that happened to be
//! clean — would make those later tests pass for the wrong reason.
//!
//! So this file asserts the shape first — the branch arrives with `git clone`, the tracker
//! is at its root, it is a genuine orphan, the working tree is dirty on another branch, and
//! the temp repositories go away however the test ends.
//!
//! Then it asserts what resolution makes of that shape: the conventional ref is found
//! without being named, an `issues/` directory still beats it, and `--ref` overrides it and
//! refuses rather than falls back. Reading the resolved ref is a later task, so those
//! assertions read the refusal to learn *which* tracker was chosen.

// Tests assert; that is their job. The crate denies unwrap/expect/panic because a malformed
// tracker must produce a diagnostic rather than a stack trace, but a test that cannot panic
// cannot fail.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::{Scenario, TRACKER_BRANCH, TmpDir, WORK_BRANCH, git, have_git, trck, trck_must};

#[test]
fn the_clone_has_the_tracker_branch_and_its_root_is_the_tracker() {
    let Some(s) = Scenario::build("ref-shape") else {
        return; // no git; see `common::have_git`
    };

    // Fetched by the clone without asking, which is the whole argument for a branch over a
    // ref outside `refs/heads/`: `git clone` brings it along.
    let config = s.show(&format!("origin/{TRACKER_BRANCH}"), "trck.json").expect("trck.json on the tracker branch");
    assert!(config.contains("format"), "not a tracker config:\n{config}");

    let index = s.show(&format!("origin/{TRACKER_BRANCH}"), "index.jsonl").expect("index.jsonl on the tracker branch");
    for id in ["aaaaaaa", "bbbbbbb"] {
        assert!(index.contains(id), "{id} missing from the seeded index:\n{index}");
    }

    // At the *root*, not under `issues/` — the path the ref layer will resolve.
    assert!(s.show(&format!("origin/{TRACKER_BRANCH}"), "issues/index.jsonl").is_none(), "tracker is nested, not at the branch root");
}

#[test]
fn the_tracker_branch_shares_no_history_with_the_code() {
    let Some(s) = Scenario::build("ref-orphan") else {
        return;
    };
    // An orphan branch has no merge base with `main`. If it ever gains one, the fixture has
    // stopped modelling the design and every later assertion is about something else.
    assert!(!common::git_ok(&s.work, &["merge-base", "origin/main", &format!("origin/{TRACKER_BRANCH}")]), "the tracker branch is not an orphan");
    assert!(s.show("origin/main", "README.md").is_some(), "main lost its code");
}

#[test]
fn the_working_tree_is_dirty_and_on_an_unrelated_branch() {
    let Some(s) = Scenario::build("ref-dirty") else {
        return;
    };
    // Both halves matter: a read that only works from a clean tree on `main` has not
    // demonstrated the property the ref layer exists for.
    assert_eq!(git(&s.work, &["rev-parse", "--abbrev-ref", "HEAD"]), WORK_BRANCH);
    assert!(!git(&s.work, &["status", "--porcelain"]).is_empty(), "working tree is clean");
}

/// The inversion this branch exists to cause.
///
/// Before the ref step, walking up from a checkout with no `issues/` found nothing and said
/// so. Now the conventional ref is found without being named, from a dirty tree on an
/// unrelated branch — which is the whole claim. Reading it is a later task, so what the
/// refusal proves is *which tracker was resolved*, and that it was not the walk-up.
#[test]
fn the_conventional_ref_is_resolved_without_being_named() {
    let Some(s) = Scenario::build("ref-conventional") else {
        return;
    };
    let out = trck(&s.work, &["list"]);
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains(TRACKER_BRANCH), "the conventional ref was not resolved: {err}");
    assert!(!err.contains("no tracker found"), "fell back to the walk-up: {err}");
}

/// The staging rule: a checkout keeps behaving exactly as it did until its `issues/` goes
/// away. Without this the epic could not land in pieces — every intermediate commit would
/// change what an ordinary `trck list` does in this very repository.
#[test]
fn a_working_tree_tracker_beats_the_conventional_ref() {
    let Some(s) = Scenario::build("ref-dir-wins") else {
        return;
    };
    trck_must(&s.work, &["init", "issues"]);
    trck_must(&s.work, &["--dir", "issues", "new", "Local", "--id", "ccccccc"]);

    let out = trck(&s.work, &["list"]);
    assert!(out.status.success(), "the working-tree tracker lost to the ref: {}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("ccccccc"), "listed something other than the working-tree tracker:\n{stdout}");
    // The seeded ids live only on the branch; seeing one would mean the ref won.
    assert!(!stdout.contains("aaaaaaa"), "read the ref instead of the directory:\n{stdout}");
}

/// `--ref` overrides the convention, and — like `--dir` — a name that does not resolve is
/// an error rather than a quiet fall back to whatever discovery would have found.
#[test]
fn an_explicit_ref_overrides_the_convention_and_does_not_fall_back() {
    let Some(s) = Scenario::build("ref-explicit") else {
        return;
    };
    let named = trck(&s.work, &["--ref", &format!("origin/{TRACKER_BRANCH}"), "list"]);
    let err = String::from_utf8_lossy(&named.stderr);
    assert!(err.contains(&format!("origin/{TRACKER_BRANCH}")), "the named ref was not the one resolved: {err}");

    let bogus = trck(&s.work, &["--ref", "no-such-ref", "list"]);
    assert!(!bogus.status.success(), "a ref that does not exist was accepted");
    let err = String::from_utf8_lossy(&bogus.stderr);
    assert!(err.contains("no-such-ref"), "the refusal must name the ref: {err}");
    assert!(!err.contains(TRACKER_BRANCH), "fell back to the convention: {err}");
}

#[test]
fn a_temp_repo_is_removed_when_an_assertion_fails() {
    if !have_git() {
        return;
    }
    // The failure this guards against is a cleanup line at the end of a test body, which is
    // exactly the code that does not run when the test fails — so the temp repositories
    // survive on the runs someone is about to repeat.
    let leaked = std::panic::catch_unwind(|| {
        let tmp = TmpDir::new("ref-drop");
        let path = tmp.path().to_path_buf();
        // Hand the path out before unwinding, so the assertion below can look for it.
        std::panic::panic_any(path);
    })
    .expect_err("the closure must panic");

    let path = leaked.downcast::<std::path::PathBuf>().expect("the panic payload is the temp path");
    assert!(!path.exists(), "temp dir survived a panic: {}", path.display());
}

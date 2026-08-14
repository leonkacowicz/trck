//! The ref-backed tracker harness, checked against itself.
//!
//! Every task in the reads and writes tranches of `#sqzr7nk` asserts through
//! [`common::Scenario`], so the fixture it builds is load-bearing before any of them
//! exist. A harness that quietly built the wrong shape — a tracker committed on `main`
//! after all, a clone that never fetched the branch, a working tree that happened to be
//! clean — would make those later tests pass for the wrong reason.
//!
//! So this file asserts the shape, and the two properties the shape is useless without:
//! the tracker is reachable *only* through the ref, and the temp repositories go away
//! however the test ends.

// Tests assert; that is their job. The crate denies unwrap/expect/panic because a malformed
// tracker must produce a diagnostic rather than a stack trace, but a test that cannot panic
// cannot fail.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::{Scenario, TRACKER_BRANCH, TmpDir, WORK_BRANCH, git, have_git, trck};

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

#[test]
fn the_tracker_is_not_reachable_from_the_working_tree() {
    let Some(s) = Scenario::build("ref-unreachable") else {
        return;
    };
    // Today's discovery walks up and finds nothing, which is exactly why the ref layer is
    // needed. When `#3dv63bn` lands this assertion inverts — and that inversion is the
    // point, so it is asserted rather than left implicit.
    let out = trck(&s.work, &["list"]);
    assert!(!out.status.success(), "a tracker was found in a checkout that has none");
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("no tracker found"), "unexpected refusal: {err}");
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

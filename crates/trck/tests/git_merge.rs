//! The merge drivers, driven by **real git merges**.
//!
//! Everything else about `merge-index` is covered by unit tests and conformance fixtures,
//! which call it directly with three files. That is not the same thing. The driver exists to
//! be invoked *by git, mid-merge*, with operands git chose, in a worktree that is not yet
//! the merged result — and the failure this code prevents (a rollup derived from a
//! half-merged index, a conflict laundered into a plausible file) only appears when git is
//! the one calling. So these tests build a repository, branch it, and merge.
//!
//! Skipped when `git` is absent, the way `app_js.rs` skips without node.

// Tests assert; that is their job. The crate denies unwrap/expect/panic because a malformed
// tracker must produce a diagnostic rather than a stack trace, but a test that cannot panic
// cannot fail.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::Path;
use std::process::Command;

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git").args(args).current_dir(dir).output().unwrap_or_else(|e| panic!("git {args:?}: {e}"));
    String::from_utf8_lossy(&out.stdout).to_string()
}

fn git_ok(dir: &Path, args: &[&str]) -> bool {
    Command::new("git").args(args).current_dir(dir).output().is_ok_and(|o| o.status.success())
}

fn trck(dir: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_trck"))
        .args(args)
        .current_dir(dir)
        .env("TRCK_NOW", "2026-01-01T00:00:00Z")
        .env("NO_COLOR", "1")
        .output()
        .expect("running trck")
}

fn have_git() -> bool {
    Command::new("git").arg("--version").output().is_ok()
}

/// A repo with a tracker, one committed issue, and the drivers registered.
fn setup(root: &Path) {
    std::fs::create_dir_all(root.join("issues/items")).expect("mkdir");
    std::fs::write(root.join("issues/trck.json"), "{}\n").expect("write config");
    assert!(git_ok(root, &["init", "-q"]), "git init");
    git(root, &["config", "user.email", "t@example.test"]);
    git(root, &["config", "user.name", "trck test"]);
    let r = trck(root, &["--dir", "issues", "new", "Shared", "--id", "aaaaaaa"]);
    assert!(r.status.success(), "seed: {}", String::from_utf8_lossy(&r.stderr));
    git(root, &["add", "-A"]);
    git(root, &["commit", "-qm", "init"]);
    let r = trck(root, &["--dir", "issues", "repo", "setup-git"]);
    assert!(r.status.success(), "setup-git: {}", String::from_utf8_lossy(&r.stderr));
    git(root, &["add", "-A"]);
    git(root, &["commit", "-qm", "setup"]);
}

/// Branch off, run `f`, commit, and return to the starting branch.
fn on_branch(root: &Path, name: &str, base: &str, f: impl Fn()) {
    git(root, &["checkout", "-q", "-b", name, base]);
    f();
    git(root, &["add", "-A"]);
    git(root, &["commit", "-qm", name]);
}

fn current_branch(root: &Path) -> String {
    git(root, &["rev-parse", "--abbrev-ref", "HEAD"]).trim().to_string()
}

#[test]
fn git_auto_resolves_disjoint_creations_through_the_driver() {
    if !have_git() {
        return;
    }
    let tmp = std::env::temp_dir().join("trck-merge-disjoint");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).expect("mkdir");
    setup(&tmp);
    let main = current_branch(&tmp);

    on_branch(&tmp, "feature", &main, || {
        trck(&tmp, &["--dir", "issues", "new", "From feature", "--id", "bbbbbbb"]);
    });
    git(&tmp, &["checkout", "-q", &main]);
    trck(&tmp, &["--dir", "issues", "new", "From main", "--id", "ccccccc"]);
    git(&tmp, &["add", "-A"]);
    git(&tmp, &["commit", "-qm", "main"]);

    assert!(git_ok(&tmp, &["merge", "feature", "-m", "merged"]), "git could not merge; the driver should have resolved this");

    // Both sides' rows survive, and the file is still a clean index.
    let index = std::fs::read_to_string(tmp.join("issues/index.jsonl")).expect("index");
    for id in ["aaaaaaa", "bbbbbbb", "ccccccc"] {
        assert!(index.contains(id), "{id} missing from merged index:\n{index}");
    }
    assert!(!index.contains("<<<<<<<"), "clean merge left markers:\n{index}");

    // And the rollup was regenerated from the merged rows, not left stale. During the merge
    // the working-tree index was not yet the merged result, so a driver that re-read it
    // would have produced a summary missing one side.
    let summary = std::fs::read_to_string(tmp.join("issues/SUMMARY.md")).expect("summary");
    assert!(summary.contains("From feature"), "summary stale:\n{summary}");
    assert!(summary.contains("From main"), "summary stale:\n{summary}");

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn git_reports_a_lifecycle_conflict_and_leaves_the_summary_alone() {
    if !have_git() {
        return;
    }
    let tmp = std::env::temp_dir().join("trck-merge-conflict");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).expect("mkdir");
    setup(&tmp);
    let main = current_branch(&tmp);

    // One side works the issue, the other closes it: `(status, closed, resolution)` cannot
    // merge as a unit, and no field-wise rule would catch it.
    on_branch(&tmp, "feature", &main, || {
        trck(&tmp, &["--dir", "issues", "start", "aaaaaaa"]);
    });
    git(&tmp, &["checkout", "-q", &main]);
    trck(&tmp, &["--dir", "issues", "done", "aaaaaaa"]);
    git(&tmp, &["add", "-A"]);
    git(&tmp, &["commit", "-qm", "main"]);

    // The baseline is the rollup as it stands *immediately before the merge* — the verbs
    // above rewrote it legitimately, and comparing against anything earlier would test them
    // rather than the driver.
    let before = std::fs::read_to_string(tmp.join("issues/SUMMARY.md")).expect("summary");

    assert!(!git_ok(&tmp, &["merge", "feature", "-m", "merged"]), "git reported success on a lifecycle conflict");

    // The file carries markers, so it cannot be `git add`ed unread: any trck verb fails on it.
    let index = std::fs::read_to_string(tmp.join("issues/index.jsonl")).expect("index");
    assert!(index.contains("<<<<<<<"), "no conflict markers:\n{index}");
    assert!(index.contains("one side"), "markers not labelled:\n{index}");
    for word in ["ours", "theirs", "yours"] {
        assert!(!index.to_lowercase().contains(word), "markers name a side ({word}), which reverses between merge and rebase:\n{index}");
    }

    // The rollup is untouched: regenerating from a half-merged index would launder the
    // conflict into a plausible-looking file, and a stale rollup is obvious where a
    // fabricated one is not.
    let after = std::fs::read_to_string(tmp.join("issues/SUMMARY.md")).expect("summary");
    assert_eq!(after, before, "SUMMARY.md was rewritten during a conflict");

    let _ = std::fs::remove_dir_all(&tmp);
}

//! `repo setup-git` and `repo install-hook`, against **real git repositories**.
//!
//! Both verbs write into someone else's territory — `.git/config`, `.git/hooks`, a committed
//! `.gitattributes` — and neither can be checked by inspecting a return value. What matters is
//! whether git *then behaves differently*: whether it finds the drivers, and whether the hook
//! actually stops a commit. So these build repositories and commit into them.
//!
//! The conformance suite cannot cover this: its `setup` lines exec only the trck binary, so a
//! fixture has no way to `git init`. Skipped when `git` is absent, the way `app_js.rs` skips
//! without node.

// Tests assert; that is their job. The crate denies unwrap/expect/panic because a malformed
// tracker must produce a diagnostic rather than a stack trace, but a test that cannot panic
// cannot fail.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn git(dir: &Path, args: &[&str]) -> Output {
    Command::new("git").args(args).current_dir(dir).output().unwrap_or_else(|e| panic!("git {args:?}: {e}"))
}

fn git_out(dir: &Path, args: &[&str]) -> String {
    String::from_utf8_lossy(&git(dir, args).stdout).trim().to_string()
}

fn trck(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_trck"))
        .args(args)
        .current_dir(dir)
        .env("TRCK_NOW", "2026-01-01T00:00:00Z")
        .env("NO_COLOR", "1")
        .output()
        .expect("running trck")
}

fn ok(dir: &Path, args: &[&str]) -> String {
    let r = trck(dir, args);
    assert!(r.status.success(), "trck {args:?} failed: {}", String::from_utf8_lossy(&r.stderr));
    String::from_utf8_lossy(&r.stdout).to_string()
}

fn have_git() -> bool {
    Command::new("git").arg("--version").output().is_ok()
}

/// A fresh directory nobody else is using, removed first so a crashed run cannot poison the
/// next one.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("trck-hooks-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("mkdir");
    dir
}

/// A git repo with a tracker at `rel` holding one issue. `rel` of "." puts the tracker at the
/// repo root, which is the case the hook's guard has to special-case.
fn repo_with_tracker(name: &str, rel: &str) -> PathBuf {
    let root = scratch(name);
    let tracker = if rel == "." { root.clone() } else { root.join(rel) };
    std::fs::create_dir_all(tracker.join("items")).expect("mkdir");
    std::fs::write(tracker.join("trck.json"), "{}\n").expect("write config");
    assert!(git(&root, &["init", "-q"]).status.success(), "git init");
    git(&root, &["config", "user.email", "t@example.test"]);
    git(&root, &["config", "user.name", "trck test"]);
    ok(&root, &["--dir", rel, "new", "One", "--id", "aaaaaaa"]);
    root
}

#[test]
fn setup_git_declares_the_drivers_and_registers_them_in_this_clone() {
    if !have_git() {
        return;
    }
    let root = repo_with_tracker("declare", "issues");
    let out = ok(&root, &["--dir", "issues", "repo", "setup-git"]);
    assert!(out.contains("registered merge drivers"), "{out}");

    // The shared half: committed, so every clone sees which drivers to use.
    let attrs = std::fs::read_to_string(root.join("issues/.gitattributes")).expect("gitattributes");
    for want in ["index.jsonl merge=trck-index text eol=lf", "SUMMARY.md merge=trck-summary text eol=lf", "items/*.md text eol=lf"] {
        assert!(attrs.lines().any(|l| l == want), "missing {want:?} in:\n{attrs}");
    }

    // The per-clone half: git never shares these, because that would make cloning a repo
    // remote code execution. An absolute engine path, never a bare `trck` — the driver fires
    // much later, from whatever environment git happens to have.
    let driver = git_out(&root, &["config", "--get", "merge.trck-index.driver"]);
    assert!(driver.contains("repo merge-index %O %A %B"), "{driver}");
    assert!(driver.starts_with('"') && driver.contains(env!("CARGO_BIN_EXE_trck")), "driver is not an absolute path to this engine: {driver}");
    assert!(git_out(&root, &["config", "--get", "merge.trck-summary.driver"]).contains("repo merge-summary %A"));

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn setup_git_run_twice_changes_nothing_the_second_time() {
    if !have_git() {
        return;
    }
    let root = repo_with_tracker("idempotent", "issues");
    ok(&root, &["--dir", "issues", "repo", "setup-git"]);
    let first = std::fs::read_to_string(root.join("issues/.gitattributes")).expect("gitattributes");

    let out = ok(&root, &["--dir", "issues", "repo", "setup-git"]);
    assert!(out.contains("already declares the trck drivers"), "{out}");
    let second = std::fs::read_to_string(root.join("issues/.gitattributes")).expect("gitattributes");
    assert_eq!(first, second, "the file was rewritten on a second run");

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn setup_git_adds_its_rules_beside_someone_elses() {
    if !have_git() {
        return;
    }
    let root = repo_with_tracker("coexist", "issues");
    std::fs::write(root.join("issues/.gitattributes"), "*.png binary\nindex.jsonl -diff\n").expect("seed");
    ok(&root, &["--dir", "issues", "repo", "setup-git"]);

    let attrs = std::fs::read_to_string(root.join("issues/.gitattributes")).expect("gitattributes");
    // A rule carrying anything we do not manage is somebody's decision: ours goes beside it
    // and git resolves the pair.
    assert!(attrs.lines().any(|l| l == "*.png binary"), "{attrs}");
    assert!(attrs.lines().any(|l| l == "index.jsonl -diff"), "{attrs}");
    assert!(attrs.lines().any(|l| l.contains("merge=trck-index")), "{attrs}");

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn the_installed_hook_stops_a_commit_that_breaks_the_tracker() {
    if !have_git() {
        return;
    }
    let root = repo_with_tracker("enforce", "issues");
    let installed = ok(&root, &["--dir", "issues", "repo", "install-hook"]);
    assert!(installed.contains("pre-commit"), "{installed}");

    // A consistent tracker commits normally — the hook has to be invisible when nothing is
    // wrong, or it just gets disabled.
    git(&root, &["add", "-A"]);
    assert!(git(&root, &["commit", "-qm", "init"]).status.success(), "the hook rejected a consistent tracker");

    // Now break it the only way that matters: by hand, which is exactly what the hook exists
    // to catch, since no verb would ever write this.
    std::fs::write(root.join("issues/index.jsonl"), "{\"id\": \"aaaaaaa\", \"slug\": \"one\", \"title\": \"One\", \"status\": \"nonsense\"}\n").expect("write");
    git(&root, &["add", "-A"]);
    let r = git(&root, &["commit", "-qm", "broken"]);
    assert!(!r.status.success(), "the hook let a broken tracker through");
    let combined = String::from_utf8_lossy(&r.stdout).to_string() + &String::from_utf8_lossy(&r.stderr);
    assert!(combined.contains("trck inconsistent"), "the hook failed for some other reason:\n{combined}");

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
#[cfg(unix)]
fn the_installed_hook_is_executable() {
    use std::os::unix::fs::PermissionsExt;
    if !have_git() {
        return;
    }
    let root = repo_with_tracker("mode", "issues");
    ok(&root, &["--dir", "issues", "repo", "install-hook"]);
    // Git silently ignores a hook without the bit, which is the worst possible failure: the
    // check appears installed and never runs.
    let mode = std::fs::metadata(root.join(".git/hooks/pre-commit")).expect("hook").permissions().mode();
    assert_eq!(mode & 0o111, 0o111, "hook is not executable (mode {mode:o})");

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_tracker_at_the_repo_root_fires_on_any_staged_change() {
    if !have_git() {
        return;
    }
    let root = repo_with_tracker("rootlevel", ".");
    ok(&root, &["--dir", ".", "repo", "install-hook"]);
    let body = std::fs::read_to_string(root.join(".git/hooks/pre-commit")).expect("hook");

    // With the tracker at the root there is no path prefix to grep for: git's staged paths are
    // repo-relative, so a `(^|/)./` guard would never match and the hook would silently never
    // run. The whole repo is the tracker, so it fires on anything.
    assert!(body.contains("if [ -n \"$staged\" ]; then"), "root-level guard missing:\n{body}");
    assert!(!body.contains("grep -qE"), "root-level hook still greps for a prefix:\n{body}");

    // And it is enforcing, not just well-formed.
    std::fs::write(root.join("index.jsonl"), "{\"id\": \"aaaaaaa\", \"status\": \"nonsense\"}\n").expect("write");
    git(&root, &["add", "-A"]);
    assert!(!git(&root, &["commit", "-qm", "broken"]).status.success(), "root-level hook did not fire");

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_subdirectory_tracker_only_fires_on_its_own_paths() {
    if !have_git() {
        return;
    }
    let root = repo_with_tracker("scoped", "issues");
    ok(&root, &["--dir", "issues", "repo", "install-hook"]);
    let body = std::fs::read_to_string(root.join(".git/hooks/pre-commit")).expect("hook");
    assert!(body.contains("grep -qE '(^|/)issues/'"), "guard does not scope to the tracker:\n{body}");
    assert!(body.contains("--dir \"$root/issues\""), "hook does not point at the tracker:\n{body}");

    // A commit touching nothing in the tracker skips the check entirely — even while the
    // tracker is broken, which is what proves the guard is doing the skipping.
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-qm", "init"]);
    std::fs::write(root.join("issues/index.jsonl"), "{\"bad\": true}\n").expect("write");
    std::fs::write(root.join("README.md"), "unrelated\n").expect("write");
    git(&root, &["add", "README.md"]);
    assert!(git(&root, &["commit", "-qm", "unrelated"]).status.success(), "an unrelated commit was blocked by the tracker hook");

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn both_verbs_refuse_outside_a_git_repository() {
    if !have_git() {
        return;
    }
    // A tracker is not enough; these verbs write into .git, and there is none.
    let dir = scratch("nogit");
    std::fs::create_dir_all(dir.join("items")).expect("mkdir");
    std::fs::write(dir.join("trck.json"), "{}\n").expect("write config");

    for verb in ["setup-git", "install-hook"] {
        let r = trck(&dir, &["--dir", ".", "repo", verb]);
        assert!(!r.status.success(), "{verb} succeeded outside a repo");
        let err = String::from_utf8_lossy(&r.stderr);
        assert!(err.contains("not a git repository"), "{verb}: {err}");
    }

    let _ = std::fs::remove_dir_all(&dir);
}

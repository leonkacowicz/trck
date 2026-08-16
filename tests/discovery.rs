//! End-to-end coverage for the boundary between implicit filesystem discovery and explicit
//! tracker selection.

#![allow(clippy::expect_used)]

mod common;

use common::{TmpDir, git_must, have_git, trck};
use std::path::Path;
use std::process::Command;

fn init_repo(path: &Path) {
    std::fs::create_dir_all(path).expect("mkdir");
    git_must(path, &["init", "-q", "-b", "main"]);
    std::fs::write(path.join("README.md"), "# fixture\n").expect("write");
    git_must(path, &["add", "-A"]);
    git_must(path, &["commit", "-qm", "initial"]);
}

fn tracker_branch(path: &Path) {
    init_repo(path);
    git_must(path, &["checkout", "-q", "--orphan", "trck-issues"]);
    git_must(path, &["rm", "-rq", "--cached", "."]);
    std::fs::remove_file(path.join("README.md")).expect("rm");
    let out = trck(path, &["init", "."]);
    assert!(out.status.success(), "trck init: {}", String::from_utf8_lossy(&out.stderr));
}

fn assert_bounded(cwd: &Path, foreign_tracker: &Path) {
    let out = trck(cwd, &["list"]);
    assert!(!out.status.success(), "discovery adopted {}", foreign_tracker.display());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("repository root"), "missing boundary in diagnostic: {err}");
    let root = git_must(cwd, &["rev-parse", "--show-toplevel"]);
    assert!(err.contains(&root), "diagnostic does not name the boundary {root}: {err}");
}

#[test]
fn discovery_does_not_adopt_a_sibling_repositorys_checked_out_tracker_branch() {
    if !have_git() {
        return;
    }
    let root = TmpDir::new("discovery-siblings");
    let repo = root.path().join("repo");
    let sibling = root.path().join("sibling");
    init_repo(&repo);
    tracker_branch(&sibling);

    assert_bounded(&repo, &sibling);
}

#[test]
fn a_linked_worktree_uses_its_own_checkout_root_as_the_boundary() {
    if !have_git() {
        return;
    }
    let root = TmpDir::new("discovery-worktree");
    let repo = root.path().join("repo");
    let worktree = root.path().join("worktree");
    let tracker = root.path().join("tracker");
    init_repo(&repo);
    git_must(&repo, &["worktree", "add", "-q", "-b", "linked", &worktree.display().to_string()]);
    tracker_branch(&tracker);

    assert_bounded(&worktree, &tracker);
}

#[test]
fn explicit_directory_overrides_can_reach_outside_the_repository() {
    if !have_git() {
        return;
    }
    let root = TmpDir::new("discovery-explicit");
    let repo = root.path().join("repo");
    let tracker = root.path().join("tracker");
    init_repo(&repo);
    std::fs::create_dir_all(&tracker).expect("mkdir tracker");
    std::fs::write(tracker.join("trck.json"), "{}\n").expect("config");

    for (args, env_dir) in [(vec!["--dir", tracker.to_str().expect("utf8"), "version"], None), (vec!["version"], Some(&tracker))] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_trck"));
        command.args(args).current_dir(&repo).env("NO_COLOR", "1");
        if let Some(dir) = env_dir {
            command.env("TRCK_DIR", dir);
        }
        let out = command.output().expect("running trck");
        assert!(out.status.success(), "explicit override failed: {}", String::from_utf8_lossy(&out.stderr));
        assert!(String::from_utf8_lossy(&out.stderr).contains(&tracker.display().to_string()));
    }
}

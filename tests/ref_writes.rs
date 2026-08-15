//! Writing a tracker that lives in a git ref.
//!
//! The claim under test is that a write verb produces **one commit** whose tree is exactly
//! what the directory backend would have written, without checking anything out — so the
//! operator's branch, working tree and index are the same afterwards as before.
//!
//! Everything runs against [`common::Scenario`]: a clone sitting on an unrelated branch with
//! an uncommitted edit, whose only tracker ref is the remote-tracking one. That last part is
//! what makes the first write here the *first write* case rather than an advance.

// Tests assert; that is their job. The crate denies unwrap/expect/panic because a malformed
// tracker must produce a diagnostic rather than a stack trace, but a test that cannot panic
// cannot fail.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::{SEEDED_BODY, Scenario, TRACKER_BRANCH, TmpDir, WORK_BRANCH, git, git_must, trck_must};
use std::path::Path;
use std::process::{Command, Output};

/// The local branch a write lands on.
const LOCAL_REF: &str = "refs/heads/trck-issues";

/// Every file the revision holds, as `path\tblob` lines — the whole tree, flattened.
fn tree(dir: &Path, rev: &str) -> Vec<String> {
    git_must(dir, &["ls-tree", "-r", "--name-only", rev]).lines().map(str::to_string).collect()
}

/// A directory tracker seeded exactly the way the fixture's ref-backed one was.
///
/// Same commands, same pinned clock, same ids: the point of the comparison is the *backend*,
/// so everything else has to be identical or the diff is measuring the fixture.
///
/// In a temp directory of its own, **not** inside the scenario's. The fixture goes out of its
/// way to leave nothing above the clone for the walk-up to find, and a tracker directory
/// parked there is one discovery resolves in preference to the ref — so the ref-backed half
/// of this very comparison would quietly write to it instead, and the two sides would agree
/// for the worst possible reason.
fn seeded_dir(home: &TmpDir) -> std::path::PathBuf {
    let dir = home.path().join("as-a-directory");
    std::fs::create_dir_all(&dir).expect("mkdir");
    trck_must(&dir, &["init", "."]);
    trck_must(&dir, &["--dir", ".", "new", "Seeded issue", "--id", "aaaaaaa", "--body", SEEDED_BODY]);
    trck_must(&dir, &["--dir", ".", "new", "Second issue", "--id", "bbbbbbb", "--empty"]);
    dir
}

/// Run the binary against a clone with no commit identity reachable from anywhere.
///
/// Every source has to go, and each needs a different mechanism: the repository's own config
/// is unset, the global and system files are pointed at an empty file of the fixture's own,
/// and `user.useConfigOnly` stops git inventing one from the passwd entry and the hostname —
/// which some runners let it do, and which would make this test pass or fail by machine.
///
/// **An empty file, not `/dev/null`**: that path does not exist on Windows, where nulling the
/// config this way would quietly null nothing.
fn trck_without_identity(work: &Path, empty_config: &Path, args: &[&str]) -> Output {
    git_must(work, &["config", "--unset", "user.email"]);
    git_must(work, &["config", "--unset", "user.name"]);
    git_must(work, &["config", "user.useConfigOnly", "true"]);
    Command::new(env!("CARGO_BIN_EXE_trck"))
        .args(args)
        .current_dir(work)
        .env("TRCK_NOW", "2026-01-01T00:00:00Z")
        .env("NO_COLOR", "1")
        .env("GIT_CONFIG_GLOBAL", empty_config)
        .env("GIT_CONFIG_SYSTEM", empty_config)
        .env_remove("GIT_AUTHOR_NAME")
        .env_remove("GIT_AUTHOR_EMAIL")
        .env_remove("GIT_COMMITTER_NAME")
        .env_remove("GIT_COMMITTER_EMAIL")
        .env_remove("EMAIL")
        .output()
        .expect("running trck")
}

/// The tracker's only ref in a fresh clone is the remote-tracking one, so the first write
/// has to *create* the local branch rather than advance it — and its parent is what the
/// remote holds, or the issue just filed would not descend from the tracker it was filed in.
#[test]
fn the_first_write_creates_the_local_branch_on_top_of_the_remote() {
    let Some(s) = Scenario::build("refwrite-first") else { return };
    assert!(!git_ok_ref(&s.work, LOCAL_REF), "the fixture must start without a local tracker branch");
    let remote_tip = git_must(&s.work, &["rev-parse", &format!("origin/{TRACKER_BRANCH}")]);

    trck_must(&s.work, &["new", "Filed from a ref", "--id", "ccccccc", "--body", "Filed against a ref."]);

    assert!(git_ok_ref(&s.work, LOCAL_REF), "the write must anchor its commit on a ref");
    assert_eq!(git_must(&s.work, &["rev-parse", &format!("{LOCAL_REF}^")]), remote_tip, "and descend from what the remote held");
    assert_eq!(git_must(&s.work, &["rev-list", "--count", LOCAL_REF]), "2", "one new commit on top of the seeded one");
}

/// One verb, one commit — and the commit says what the verb was asked to do.
#[test]
fn a_write_verb_produces_one_commit_that_names_the_operation() {
    let Some(s) = Scenario::build("refwrite-one") else { return };
    trck_must(&s.work, &["new", "Filed from a ref", "--id", "ccccccc", "--body", "Filed against a ref."]);
    let before = git_must(&s.work, &["rev-parse", LOCAL_REF]);

    trck_must(&s.work, &["start", "ccccccc"]);

    assert_eq!(git_must(&s.work, &["rev-parse", &format!("{LOCAL_REF}^")]), before, "the second write chains onto the first");
    let subject = git_must(&s.work, &["log", "-1", "--format=%s", LOCAL_REF]);
    assert_eq!(subject, "mv ccccccc in-progress", "the canonical operation, not the alias typed");
}

/// The acceptance criterion this whole tranche turns on: the commit's tree is what the
/// directory backend would have put on disk, file for file and byte for byte.
#[test]
fn the_commit_tree_is_what_the_directory_backend_would_have_written() {
    let Some(s) = Scenario::build("refwrite-parity") else { return };
    let home = TmpDir::new("refwrite-parity-dir");
    let dir = seeded_dir(&home);

    // The same verb against each backend.
    for args in [
        ["new", "Filed from a ref", "--id", "ccccccc", "--body", "Filed against a ref."].as_slice(),
        ["start", "ccccccc"].as_slice(),
        ["set", "ccccccc", "--title", "Renamed", "--slug", "renamed"].as_slice(),
        ["label", "ccccccc", "--add", "infra"].as_slice(),
        ["done", "ccccccc", "--resolution", "wontfix"].as_slice(),
    ] {
        trck_must(&s.work, args);
        let mut dired = vec!["--dir", "."];
        dired.extend_from_slice(args);
        trck_must(&dir, &dired);
    }

    // Same paths...
    let on_disk = {
        let mut names: Vec<String> = walk(&dir, &dir);
        names.sort();
        names
    };
    assert_eq!(tree(&s.work, LOCAL_REF), on_disk, "the tree holds exactly the files the directory does");

    // ...and the same bytes at each.
    for path in &on_disk {
        let committed = s.show(TRACKER_BRANCH, path);
        let written = std::fs::read_to_string(dir.join(path)).ok();
        assert_eq!(committed, written, "{path} differs between the backends");
    }
}

/// Every write reads the ref it is about to move, and builds on what it finds *now*.
///
/// The compare-and-swap that makes a concurrent write lose cleanly rather than clobber is
/// `update_ref`'s, and it is unit-tested there against a stale expectation. What this covers
/// is the half a single process can be held to: someone else's commit landing on the branch
/// between two writes is picked up as the parent of the next one, rather than the tip this
/// process last saw. A backend that remembered would fork the history silently.
#[test]
fn a_write_builds_on_the_ref_as_it_stands_now() {
    let Some(s) = Scenario::build("refwrite-reread") else { return };
    trck_must(&s.work, &["new", "First", "--id", "ccccccc", "--empty"]);
    let ours = git_must(&s.work, &["rev-parse", LOCAL_REF]);

    // Somebody else's write lands on the branch — an empty commit is enough; what matters
    // is that the ref no longer holds what this process last saw.
    let theirs = git_must(&s.work, &["commit-tree", &format!("{LOCAL_REF}^{{tree}}"), "-p", &ours, "-m", "someone else"]);
    git_must(&s.work, &["update-ref", LOCAL_REF, &theirs, &ours]);

    trck_must(&s.work, &["new", "Second", "--id", "ddddddd", "--empty"]);

    assert_eq!(git_must(&s.work, &["rev-parse", &format!("{LOCAL_REF}^")]), theirs, "the next write descends from their commit, not ours");
    assert_eq!(git_must(&s.work, &["rev-list", "--count", LOCAL_REF]), "4", "and the history stayed linear");
}

/// A tracker write must not be something you have to be ready for. The fixture's clone is
/// mid-feature with an uncommitted edit, and both have to survive untouched — this is the
/// property that lets the tracker leave the working tree in the first place.
#[test]
fn the_working_tree_branch_and_index_are_untouched() {
    let Some(s) = Scenario::build("refwrite-clean") else { return };
    let status_before = git_must(&s.work, &["status", "--porcelain"]);
    let head_before = git_must(&s.work, &["rev-parse", "HEAD"]);

    trck_must(&s.work, &["new", "Filed mid-feature", "--id", "ccccccc", "--empty"]);

    assert_eq!(git_must(&s.work, &["rev-parse", "--abbrev-ref", "HEAD"]), WORK_BRANCH, "still on the feature branch");
    assert_eq!(git_must(&s.work, &["rev-parse", "HEAD"]), head_before, "which has not moved");
    assert_eq!(git_must(&s.work, &["status", "--porcelain"]), status_before, "and nothing was staged or cleaned");
    assert!(!s.work.join("index.jsonl").exists(), "no tracker file was checked out");
    assert!(!s.work.join("items").exists(), "no tracker file was checked out");
}

/// git's own refusal here is four lines of shell aimed at someone committing by hand, ending
/// in `unable to auto-detect email address` — which reads as a bug in trck rather than as a
/// machine that has never had git configured.
#[test]
fn an_unset_git_identity_is_reported_with_the_config_to_set() {
    let Some(s) = Scenario::build("refwrite-identity") else { return };
    // Its own scenario, so taking the identity away is safe: every test builds a fresh one.
    let empty = s.work.with_file_name("empty-gitconfig");
    std::fs::write(&empty, "").expect("write");

    let out = trck_without_identity(&s.work, &empty, &["new", "No identity", "--id", "ccccccc", "--empty"]);

    assert!(!out.status.success(), "a commit with no identity must be refused, not made");
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("user.email"), "the refusal must name the config to set: {err}");
    assert!(err.contains("user.name"), "{err}");
    assert!(!err.contains("auto-detect"), "git's own wording is replaced, not passed through: {err}");
    assert!(!git_ok_ref(&s.work, LOCAL_REF), "and nothing was anchored");
}

/// Does the ref resolve? `git()` rather than `git_must()`, because "no" is the answer in
/// half these assertions rather than a failure.
fn git_ok_ref(dir: &Path, refname: &str) -> bool {
    !git(dir, &["rev-parse", "--verify", "--quiet", refname]).is_empty()
}

/// Every file under `dir`, as tracker-relative slash-separated paths.
fn walk(root: &Path, dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else { return out };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            out.extend(walk(root, &path));
        } else if let Ok(rel) = path.strip_prefix(root) {
            out.push(rel.components().filter_map(|c| c.as_os_str().to_str()).collect::<Vec<&str>>().join("/"));
        }
    }
    out
}

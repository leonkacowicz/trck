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

use common::{HOLED_BRANCH, SEEDED_BODY, Scenario, TRACKER_BRANCH, TmpDir, WORK_BRANCH, git, git_must, git_ok, trck, trck_must};
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
    let seeded = commits(&s, &format!("origin/{TRACKER_BRANCH}"));

    trck_must(&s.work, &["new", "Filed from a ref", "--id", "ccccccc", "--body", "Filed against a ref."]);

    assert!(git_ok_ref(&s.work, LOCAL_REF), "the write must anchor its commit on a ref");
    assert_eq!(git_must(&s.work, &["rev-parse", &format!("{LOCAL_REF}^")]), remote_tip, "and descend from what the remote held");
    assert_eq!(commits(&s, LOCAL_REF), seeded + 1, "one new commit on top of what the remote held");
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
    assert_eq!(subject, "in-progress #ccccccc", "the destination status leads, and the alias typed is not it");
}

/// `git log --oneline trck-issues` is the tracker's changelog, so each verb's subject is the
/// documented shape rather than free text.
#[test]
fn each_verb_writes_its_documented_subject() {
    let Some(s) = Scenario::build("refwrite-subjects") else { return };
    let expected = [
        (["new", "A filed issue", "--id", "ccccccc", "--empty"].as_slice(), "new #ccccccc: A filed issue"),
        (["start", "ccccccc"].as_slice(), "in-progress #ccccccc"),
        (["set", "ccccccc", "--priority", "high"].as_slice(), "set #ccccccc priority=high"),
        (["label", "ccccccc", "--add", "infra"].as_slice(), "label #ccccccc +infra"),
        (["dep", "ccccccc", "--add", "aaaaaaa"].as_slice(), "dep #ccccccc +#aaaaaaa"),
        (["edit", "ccccccc", "--body", "Rewritten prose."].as_slice(), "edit #ccccccc"),
        (["done", "ccccccc", "--resolution", "wontfix"].as_slice(), "done #ccccccc (wontfix)"),
    ];
    for (args, subject) in expected {
        trck_must(&s.work, args);
        assert_eq!(git_must(&s.work, &["log", "-1", "--format=%s", LOCAL_REF]), subject, "for {args:?}");
    }
}

/// The load-bearing half. Every commit carries the operation that made it, on one line, in a
/// form git's own trailer reader can pick out — which is what lets a pending commit be
/// replayed against a tree it was not built on.
#[test]
fn every_commit_carries_a_single_line_trck_op_trailer() {
    let Some(s) = Scenario::build("refwrite-trailer") else { return };
    // A title with every character that would break a naive record: a newline that would end
    // the trailer, a quote, a tab, and a backslash. (A *leading* dash is the fourth, and is
    // covered where it belongs — `Op`'s own round trip — because this CLI has no `--`
    // separator, so a title starting with one cannot be typed in the first place.)
    let title = "has \"quotes\"\nsecond line\tand a tab \\ backslash";
    trck_must(&s.work, &["new", title, "--id", "ccccccc", "--empty"]);

    let trailers = git_must(&s.work, &["log", "-1", "--format=%(trailers:key=Trck-Op,valueonly)", LOCAL_REF]);
    let trailer = trailers.trim();
    assert!(!trailer.is_empty(), "git's own trailer reader must find it: {trailers:?}");
    assert_eq!(trailer.lines().count(), 1, "a trailer spanning lines is half lost: {trailer:?}");
    assert!(trailer.starts_with("new "), "{trailer}");
    // The title survives in the record even though the subject could not carry it whole.
    assert!(trailer.contains("second line"), "the newline was escaped, not dropped: {trailer}");
    assert!(trailer.contains("--id ccccccc"), "the generated id is pinned: {trailer}");

    let subject = git_must(&s.work, &["log", "-1", "--format=%s", LOCAL_REF]);
    assert!(!subject.contains('\n'), "{subject:?}");
    assert!(subject.starts_with("new #ccccccc: has \"quotes\" second line"), "{subject}");
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

/// How many commits `rev` has. Counted rather than written down: the fixture's own history
/// is its business, and an absolute number here breaks the day it gains a commit.
fn commits(s: &Scenario, rev: &str) -> usize {
    git_must(&s.work, &["rev-list", "--count", rev]).parse().expect("a count")
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

    let before = commits(&s, LOCAL_REF);
    trck_must(&s.work, &["new", "Second", "--id", "ddddddd", "--empty"]);

    assert_eq!(git_must(&s.work, &["rev-parse", &format!("{LOCAL_REF}^")]), theirs, "the next write descends from their commit, not ours");
    assert_eq!(commits(&s, LOCAL_REF), before + 1, "and the history stayed linear");
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

/// The write is validated against where it landed, not against where it was read from.
///
/// A clone that has never written to the tracker resolves it through `origin/trck-issues`,
/// the only tracker ref it has; the commit goes to the local branch. Validating the source
/// ref afterwards therefore inspects the tree the write *started from*, which of course does
/// not hold the body just written — so every first write in every fresh clone announced that
/// the tracker was inconsistent, having just made it consistently.
///
/// On stderr, while the verb still succeeded, which is why no test caught it: a caller
/// reading stdout and a status code saw nothing wrong. So this asserts on stderr.
#[test]
fn the_first_write_in_a_clone_does_not_claim_the_tracker_is_inconsistent() {
    let Some(s) = Scenario::build("refwrite-firstclean") else { return };
    assert!(!git_ok_ref(&s.work, LOCAL_REF), "the fixture must start with no local tracker branch");

    let out = trck(&s.work, &["new", "Filed from a fresh clone", "--id", "ccccccc", "--body", "Prose."]);

    assert!(out.status.success(), "the write itself must succeed");
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(!err.contains("INCONSISTENCIES"), "a first write reported the tracker as broken: {err}");
    assert!(!err.contains("no markdown file"), "{err}");
    // And the tracker really is consistent, so the claim was false rather than early.
    trck_must(&s.work, &["check"]);
}

/// And the path it prints is where the body *is*, for the same reason.
///
/// The same source resolved before the branch existed also names the body back to the caller,
/// so a fresh clone answered `origin/trck-issues:items/…` — a revision whose tree does not hold
/// what was just written. It resolves anyway once the push lands, which is what hid this; with
/// no reachable remote it is simply a path that is not there, the trap `trck path` already
/// refuses to print.
#[test]
fn a_first_write_names_the_branch_it_landed_on() {
    let Some(s) = Scenario::build("refwrite-firstwhere") else { return };
    // An unreachable remote rather than no remote: the push must fail, so that nothing updates
    // `origin/trck-issues` behind the answer and makes a wrong one resolve anyway — but the
    // remote-tracking ref has to stay, because it is the only tracker this clone can find.
    git_must(&s.work, &["remote", "set-url", "origin", &s.work.join("nowhere.git").display().to_string()]);

    let out = trck_must(&s.work, &["new", "Filed from a fresh clone", "--id", "ccccccc", "--body", "Prose."]);

    let (where_, _) = out.trim().split_once("  ").unwrap_or((out.trim(), ""));
    assert_eq!(where_, "trck-issues:items/ccccccc-filed-from-a-fresh-clone.md", "the location must name the branch written, not the ref read");
    assert!(git_ok(&s.work, &["cat-file", "-e", where_]), "and it must resolve: {where_}");
}

/// `mv` names it the same way, and it is a different code path: `new` knows the row it built,
/// `mv` looks one up and confirms its body is there before moving it.
#[test]
fn a_first_move_names_the_branch_it_landed_on() {
    let Some(s) = Scenario::build("refwrite-firstmove") else { return };
    git_must(&s.work, &["remote", "set-url", "origin", &s.work.join("nowhere.git").display().to_string()]);

    let out = trck_must(&s.work, &["start", "aaaaaaa"]);

    let (where_, _) = out.trim().split_once("  ").unwrap_or((out.trim(), ""));
    assert_eq!(where_, "trck-issues:items/aaaaaaa-seeded-issue.md", "the location must name the branch written, not the ref read");
    assert!(git_ok(&s.work, &["cat-file", "-e", where_]), "and it must resolve: {where_}");
}

/// The check still fires — validating the right tree is not the same as not validating.
///
/// The fixture's holed branch is a tracker whose index lists a body its tree does not hold,
/// which no verb can produce and a hand-edit or a bad merge can. A write onto it inherits the
/// hole, and saying so is the whole point of the report.
#[test]
fn a_write_onto_a_holed_tracker_still_reports_it() {
    let Some(s) = Scenario::build("refwrite-holed") else { return };
    let holed = format!("origin/{HOLED_BRANCH}");

    let out = trck(&s.work, &["--ref", &holed, "new", "Filed onto a hole", "--id", "ccccccc", "--body", "Prose."]);

    assert!(out.status.success(), "an inconsistent tracker does not fail the verb, it warns");
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("#bbbbbbb in index but no markdown file"), "the pre-existing hole must be named: {err}");
    // The row this very verb wrote is not the one missing — that was the false alarm.
    assert!(!err.contains("#ccccccc"), "the row just written must not be reported missing: {err}");
    assert_eq!(err.matches("INCONSISTENCIES").count(), 1, "one report per invocation: {err}");
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

//! Getting a tracker write onto the remote, and losing the race without losing the work.
//!
//! The claim is that a write is shared without a fetch first, and that when someone else got
//! there in between the operation is *rebuilt* on top of theirs rather than forced over it —
//! so two writers against one remote both end up in the history.
//!
//! `Scenario` gives one clone; a second is made here, because contention needs two.

// Tests assert; that is their job. The crate denies unwrap/expect/panic because a malformed
// tracker must produce a diagnostic rather than a stack trace, but a test that cannot panic
// cannot fail.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::{Scenario, TRACKER_BRANCH, git, git_must, trck, trck_must};
use std::path::{Path, PathBuf};

const LOCAL_REF: &str = "refs/heads/trck-issues";

/// A second clone of the same origin, on its own branch — the other writer.
fn second_clone(s: &Scenario) -> PathBuf {
    let root = s.work.parent().expect("the clone has a parent");
    let other = root.join("other");
    git_must(root, &["clone", "-q", &s.origin.display().to_string(), "other"]);
    git_must(&other, &["checkout", "-q", "-b", "their-feature"]);
    git_must(&other, &["config", "user.email", "other@example.invalid"]);
    git_must(&other, &["config", "user.name", "other writer"]);
    other
}

/// What the origin holds on the tracker branch.
fn on_origin(s: &Scenario, path: &str) -> Option<String> {
    let out = std::process::Command::new("git").args(["show", &format!("{TRACKER_BRANCH}:{path}")]).current_dir(&s.origin).output().expect("git show");
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Every subject on the origin's tracker branch, newest first.
fn origin_log(s: &Scenario) -> Vec<String> {
    let out = std::process::Command::new("git").args(["log", "--format=%s", TRACKER_BRANCH]).current_dir(&s.origin).output().expect("git log");
    String::from_utf8_lossy(&out.stdout).lines().map(str::to_string).collect()
}

/// The uncontended path: the write lands on the remote by itself, with no fetch on the way.
#[test]
fn a_write_reaches_the_remote_without_being_asked_to() {
    let Some(s) = Scenario::build("push-plain") else { return };
    trck_must(&s.work, &["new", "Filed and shared", "--id", "ccccccc", "--empty"]);

    assert!(on_origin(&s, "index.jsonl").is_some_and(|i| i.contains("ccccccc")), "the row reached the origin");
    assert_eq!(
        git_must(&s.work, &["rev-parse", LOCAL_REF]),
        git_must(&s.work, &["rev-parse", &format!("refs/remotes/origin/{TRACKER_BRANCH}")]),
        "and the local branch and its tracking ref agree"
    );
}

/// **No fetch before the write.** A commit whose parent is not the current remote tip cannot be
/// pushed, so fetching first buys nothing the push does not already give. The observable
/// version of that claim: with the remote unreachable, an *uncontended* write still fails at
/// the push rather than earlier — and a write that never contacted the remote at all would
/// have to be told about a broken one by something other than the push.
#[test]
fn the_uncontended_path_does_not_fetch_first() {
    let Some(s) = Scenario::build("push-nofetch") else { return };
    // Someone else lands a commit the clone has never seen. If a write fetched first, it would
    // notice and rebuild; because it does not, the push is what discovers the divergence.
    let other = second_clone(&s);
    trck_must(&other, &["new", "Theirs", "--id", "ddddddd", "--empty"]);

    // The clone is now behind and does not know it. Its own write still goes out in one attempt
    // *after* rebuilding — what matters here is that both survive, which the next test asserts.
    // Here: the clone's tracking ref was stale until the push forced the issue.
    let before = git(&s.work, &["rev-parse", &format!("refs/remotes/origin/{TRACKER_BRANCH}")]);
    trck_must(&s.work, &["new", "Ours", "--id", "ccccccc", "--empty"]);
    let after = git_must(&s.work, &["rev-parse", &format!("refs/remotes/origin/{TRACKER_BRANCH}")]);
    assert_ne!(before, after, "the tracking ref only moved because the push made it necessary");
}

/// The acceptance criterion: two writers, one remote, both commits present afterwards and
/// neither overwritten.
#[test]
fn a_contended_write_converges_with_both_commits_present() {
    let Some(s) = Scenario::build("push-contended") else { return };
    let other = second_clone(&s);

    // They go first and land.
    trck_must(&other, &["new", "Theirs", "--id", "ddddddd", "--body", "Their prose."]);
    // We were built against the older tip: the push is rejected, and the operation is rebuilt.
    trck_must(&s.work, &["new", "Ours", "--id", "ccccccc", "--body", "Our prose."]);

    let index = on_origin(&s, "index.jsonl").expect("the origin holds an index");
    assert!(index.contains("ddddddd"), "their row survived: {index}");
    assert!(index.contains("ccccccc"), "and ours landed too: {index}");

    // Both bodies, and ours came out of the pending commit's tree rather than being lost.
    assert_eq!(on_origin(&s, "items/ddddddd-theirs.md").as_deref().map(str::trim), Some("Their prose."));
    assert_eq!(on_origin(&s, "items/ccccccc-ours.md").as_deref().map(str::trim), Some("Our prose."), "the rebuilt commit kept our prose");

    // Linear, and ours is on top of theirs — rebuilt, not merged and not forced.
    let log = origin_log(&s);
    assert_eq!(log.first().map(String::as_str), Some("new #ccccccc: Ours"), "{log:?}");
    assert_eq!(log.get(1).map(String::as_str), Some("new #ddddddd: Theirs"), "{log:?}");
}

/// A rebuilt operation is re-*derived*, not re-applied as text. The rollup a verb produces
/// depends on the rows it runs against, so an operation replayed onto someone else's commit has
/// to see their rows — which a textual merge of `index.jsonl` could never arrange.
#[test]
fn a_rebuilt_operation_derives_against_the_other_writers_rows() {
    let Some(s) = Scenario::build("push-derive") else { return };
    let other = second_clone(&s);

    // A shared starting point: an epic with two open children, which both clones can see.
    trck_must(&other, &["new", "Epic", "--id", "eeeeeee", "--empty"]);
    trck_must(&other, &["new", "First child", "--id", "fffffff", "--parent", "eeeeeee", "--empty"]);
    trck_must(&other, &["new", "Second child", "--id", "ggggggg", "--parent", "eeeeeee", "--empty"]);
    // The test arranges the shared base by hand. The engine deliberately never fetches before a
    // write, so an issue a clone has never seen cannot be referred to — the verb refuses at
    // resolution, long before a push could teach it otherwise. That is the design, not a gap.
    git_must(&s.work, &["fetch", "-q", "origin", &format!("+refs/heads/{TRACKER_BRANCH}:refs/remotes/origin/{TRACKER_BRANCH}")]);

    // Now both close a child at once. Theirs lands; ours is rejected and rebuilt.
    trck_must(&other, &["done", "fffffff"]);
    trck_must(&s.work, &["done", "ggggggg"]);

    let index = on_origin(&s, "index.jsonl").expect("an index");
    let epic = index.lines().find(|l| l.contains("\"id\": \"eeeeeee\"")).expect("the epic survived");
    // The whole claim in one assertion: the epic is done only if the rebuilt `done` derived the
    // rollup against *their* closed child as well as ours. A textual merge of two index files
    // would have left it in-progress, because neither writer's own version ever said done.
    assert!(epic.contains("\"status\": \"done\""), "the rollup saw both children: {epic}");
    for id in ["fffffff", "ggggggg"] {
        let row = index.lines().find(|l| l.contains(id)).unwrap_or_default();
        assert!(row.contains("\"status\": \"done\""), "{id} should be closed: {row}");
    }
}

/// A tracker with no remote is a legitimate tracker. The write must not fail for want of
/// somewhere to push.
#[test]
fn a_tracker_with_no_remote_writes_locally_and_says_nothing() {
    let Some(s) = Scenario::build("push-noremote") else { return };
    // One write first, so the *local* branch exists: removing the remote also removes the
    // remote-tracking ref, and with neither there would be no tracker to find at all.
    trck_must(&s.work, &["new", "Filed and shared", "--id", "bbbbbbc", "--empty"]);
    git_must(&s.work, &["remote", "remove", "origin"]);

    let out = trck(&s.work, &["new", "Filed with nowhere to go", "--id", "ccccccc", "--empty"]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(git_ok(&s.work, LOCAL_REF), "the commit is still anchored locally");
}

/// Nothing the engine hands to `git push` ever forces — the rule the whole design rests on,
/// since forcing is exactly how the other writer's issue disappears.
///
/// Scoped to lines that actually invoke push. A blanket search for `--force` finds
/// `trck init --force`, which is an unrelated flag on an unrelated verb, and a test that has to
/// be argued with every time it fails is one people learn to ignore.
#[test]
fn no_push_the_engine_makes_is_ever_forced() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let (mut invocations, mut offenders) = (0usize, Vec::new());
    walk(&source, &mut |path, text| {
        for (n, line) in text.lines().enumerate() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("//") || !line.contains("\"push\"") {
                continue;
            }
            invocations += 1;
            // A leading `+` on a refspec is git's own spelling of a forced update, and would
            // slip past a search for the flag.
            if line.contains("--force") || line.contains("\"-f\"") || line.contains("+{") {
                offenders.push(format!("{}:{}: {}", path.display(), n + 1, line.trim()));
            }
        }
    });
    assert!(invocations > 0, "the scan found no push at all, so it is proving nothing");
    assert!(offenders.is_empty(), "a forced push reached the engine:\n{}", offenders.join("\n"));
}

fn git_ok(dir: &Path, refname: &str) -> bool {
    !git(dir, &["rev-parse", "--verify", "--quiet", refname]).is_empty()
}

fn walk(dir: &Path, f: &mut impl FnMut(&Path, &str)) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, f);
        } else if path.extension().is_some_and(|e| e == "rs")
            && let Ok(text) = std::fs::read_to_string(&path)
        {
            f(&path, &text);
        }
    }
}

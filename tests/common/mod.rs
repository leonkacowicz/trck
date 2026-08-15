//! A repository shaped the way a ref-backed tracker will find one.
//!
//! The conformance suite cannot host this. Its fixtures exec the binary against a plain
//! directory in a temp dir with no git anywhere, and that method is the reason a hosted
//! backend was ruled out — it stays as it is. So the ref layer gets an integration harness
//! here instead, modelled on `git_merge.rs`, which already stands up real repositories
//! around the binary.
//!
//! [`Scenario`] builds the whole shape in one call: a bare origin, an orphan branch whose
//! *root* is the tracker, and a clone sitting on an unrelated branch with a dirty working
//! tree. That last part is not decoration. The property the ref layer exists to have is
//! that a read answers the same thing whatever branch you are on and whatever you have
//! half-edited, so a harness whose working tree is clean and on `main` would assert almost
//! nothing.
//!
//! Everything is removed by [`Drop`], not by a line at the end of the test. A failing
//! assertion unwinds, and a cleanup that only runs on success leaves a temp repository
//! behind on exactly the runs someone is going to repeat.

// Shared by several test binaries, each of which uses part of it; the unused half would
// otherwise be a warning, and the suite is built with `-D warnings`.
#![allow(dead_code)]
// Tests assert; that is their job. The crate denies unwrap/expect/panic because a malformed
// tracker must produce a diagnostic rather than a stack trace, but a test that cannot panic
// cannot fail.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// The conventional branch a ref-backed tracker lives on.
pub(crate) const TRACKER_BRANCH: &str = "trck-issues";

/// The branch the clone is left sitting on: not `main`, because "reads do not care which
/// branch you are on" is the claim under test.
pub(crate) const WORK_BRANCH: &str = "feature";

/// The prose on the seeded issue, so a body read out of the ref is recognisable.
pub(crate) const SEEDED_BODY: &str = "Prose that lives only on the tracker branch.";

/// A tracker branch whose index lists an issue the tree no longer holds a body for.
pub(crate) const HOLED_BRANCH: &str = "trck-issues-holed";

/// Is git usable at all? Tests skip rather than fail without it, the way `app_js.rs` skips
/// without node — a contributor who has not got it should not be blocked, only uncovered.
pub(crate) fn have_git() -> bool {
    Command::new("git").arg("--version").output().is_ok_and(|o| o.status.success())
}

/// A temp directory that removes itself however the test ends.
pub(crate) struct TmpDir(PathBuf);

impl TmpDir {
    /// `std::env::temp_dir` plus pid and a counter rather than a crate — the engine takes no
    /// dependencies and its tests should not either. Per-process and per-call, because
    /// `cargo test` runs binaries concurrently and a fixed name has two of them building
    /// each other's repository.
    pub(crate) fn new(tag: &str) -> TmpDir {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static N: AtomicUsize = AtomicUsize::new(0);
        let n = N.fetch_add(1, Ordering::Relaxed);
        let p = std::env::temp_dir().join(format!("trck-{tag}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).expect("mkdir");
        TmpDir(p)
    }

    pub(crate) fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TmpDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Run git and hand back trimmed stdout, panicking on a failed spawn but not on a failed
/// command — a caller that cares uses [`git_ok`].
pub(crate) fn git(dir: &Path, args: &[&str]) -> String {
    let out = run_git(dir, args);
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// Run git, and require it to have succeeded. The panic names the command and git's own
/// words, because a harness that fails silently sends you looking in the wrong file.
pub(crate) fn git_must(dir: &Path, args: &[&str]) -> String {
    let out = run_git(dir, args);
    assert!(out.status.success(), "git {args:?} in {}: {}", dir.display(), String::from_utf8_lossy(&out.stderr).trim());
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

pub(crate) fn git_ok(dir: &Path, args: &[&str]) -> bool {
    run_git(dir, args).status.success()
}

fn run_git(dir: &Path, args: &[&str]) -> Output {
    Command::new("git")
        .args(args)
        .current_dir(dir)
        // A contributor's own identity, hooks and templates must not decide what this
        // harness builds: `commit` needs an author, and an inherited `core.hooksPath`
        // would run this repository's pre-commit hook inside the fixture.
        .env("GIT_AUTHOR_NAME", "trck test")
        .env("GIT_AUTHOR_EMAIL", "t@example.test")
        .env("GIT_COMMITTER_NAME", "trck test")
        .env("GIT_COMMITTER_EMAIL", "t@example.test")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .output()
        .unwrap_or_else(|e| panic!("git {args:?}: {e}"))
}

/// Run the binary under test, from `dir`.
pub(crate) fn trck(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_trck"))
        .args(args)
        .current_dir(dir)
        .env("TRCK_NOW", "2026-01-01T00:00:00Z")
        .env("NO_COLOR", "1")
        .output()
        .expect("running trck")
}

/// The same, but the command must have succeeded; answers with its stdout.
pub(crate) fn trck_must(dir: &Path, args: &[&str]) -> String {
    let out = trck(dir, args);
    assert!(out.status.success(), "trck {args:?}: {}", String::from_utf8_lossy(&out.stderr).trim());
    String::from_utf8_lossy(&out.stdout).to_string()
}

/// A bare origin, a clone of it, and a tracker living on an orphan branch of both.
pub(crate) struct Scenario {
    /// Holds the whole tree; dropping it removes origin and clone together.
    _root: TmpDir,
    /// The bare repository the clone pushes to.
    pub(crate) origin: PathBuf,
    /// The working clone: on [`WORK_BRANCH`], with an uncommitted edit.
    pub(crate) work: PathBuf,
}

impl Scenario {
    /// Build the shape, or `None` when git is absent so the caller can skip.
    ///
    /// The tracker is seeded through the binary rather than by writing `index.jsonl` by
    /// hand: a fixture assembled by something other than the engine is a fixture that can
    /// disagree with it, and this one is the input to every later assertion about reads.
    pub(crate) fn build(tag: &str) -> Option<Scenario> {
        if !have_git() {
            return None;
        }
        let root = TmpDir::new(tag);
        let origin = root.path().join("origin.git");
        let seed = root.path().join("seed");
        let work = root.path().join("work");
        std::fs::create_dir_all(&seed).expect("mkdir");

        std::fs::create_dir_all(&origin).expect("mkdir");
        git_must(&origin, &["init", "-q", "--bare"]);
        // Whatever the contributor's `init.defaultBranch` says, the fixture's branch names
        // are the fixture's business.
        git_must(&origin, &["symbolic-ref", "HEAD", "refs/heads/main"]);

        // `main`: code, and no tracker anywhere in the tree. That absence is the point —
        // discovery must reach the ref precisely because walking up finds nothing.
        git_must(&seed, &["init", "-q", "-b", "main"]);
        std::fs::write(seed.join("README.md"), "# fixture\n").expect("write");
        git_must(&seed, &["add", "-A"]);
        git_must(&seed, &["commit", "-qm", "code"]);
        git_must(&seed, &["remote", "add", "origin", &origin.display().to_string()]);
        git_must(&seed, &["push", "-q", "origin", "main"]);

        // The tracker branch: an orphan, so it shares no history with the code, and its
        // *root* is the tracker rather than an `issues/` inside it.
        git_must(&seed, &["checkout", "-q", "--orphan", TRACKER_BRANCH]);
        git_must(&seed, &["rm", "-rq", "--cached", "."]);
        std::fs::remove_file(seed.join("README.md")).expect("rm");
        trck_must(&seed, &["init", "."]);
        // Real prose on one of them, so a test can tell a body read out of the ref from a
        // heading the engine would have generated either way.
        trck_must(&seed, &["--dir", ".", "new", "Seeded issue", "--id", "aaaaaaa", "--body", SEEDED_BODY]);
        trck_must(&seed, &["--dir", ".", "new", "Second issue", "--id", "bbbbbbb", "--empty"]);
        git_must(&seed, &["add", "-A"]);
        git_must(&seed, &["commit", "-qm", "tracker"]);
        git_must(&seed, &["push", "-q", "origin", TRACKER_BRANCH]);

        // A second branch whose index still lists `bbbbbbb` but whose tree no longer holds
        // its body. A tracker can only reach this state through something outside the
        // verbs — a bad merge, a hand-edit — and what matters is that reading it out of a
        // ref reports the same inconsistency reading it off disk does.
        git_must(&seed, &["checkout", "-q", "-b", HOLED_BRANCH]);
        let holed = std::fs::read_dir(seed.join("items"))
            .expect("items")
            .flatten()
            .map(|e| e.path())
            .find(|p| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("bbbbbbb")))
            .expect("the seeded body");
        std::fs::remove_file(&holed).expect("rm body");
        git_must(&seed, &["add", "-A"]);
        git_must(&seed, &["commit", "-qm", "drop a body"]);
        git_must(&seed, &["push", "-q", "origin", HOLED_BRANCH]);

        // The seed has served its purpose, and it is a tracker directory sitting beside
        // the clone: left in place, discovery walks up from `work` and finds *it*, which
        // is precisely the reachability the fixture exists to deny.
        std::fs::remove_dir_all(&seed).expect("rm seed");

        // The clone: what a contributor actually has. Mid-feature, and mid-edit.
        git_must(root.path(), &["clone", "-q", &origin.display().to_string(), "work"]);
        git_must(&work, &["checkout", "-q", "-b", WORK_BRANCH]);
        std::fs::write(work.join("README.md"), "# fixture, edited and not committed\n").expect("write");

        Some(Scenario { _root: root, origin, work })
    }

    /// `<branch>:<path>` as git resolves it in the clone, or `None` when absent.
    pub(crate) fn show(&self, rev: &str, path: &str) -> Option<String> {
        let out = run_git(&self.work, &["show", &format!("{rev}:{path}")]);
        out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
    }
}

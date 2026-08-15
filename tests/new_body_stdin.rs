//! `new --body-file -` reads the body from stdin.
//!
//! The one form of `#p95aabv` the conformance suite cannot state. A fixture is one argv and
//! no standard input — the runner closes it deliberately, so that a fixture behaves the same
//! whether it runs from a shell or from CI — which leaves nothing there able to tell a
//! working pipe from a hang.
//!
//! It is also the form the acceptance criterion is really about: `--body`, `--body-file` and
//! `--body-file -` must produce *the same issue* for the same text, or they are three
//! features rather than three spellings.

// Tests assert; that is their job. The crate denies unwrap/expect/panic because a malformed
// tracker must produce a diagnostic rather than a stack trace, but a test that cannot panic
// cannot fail.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const PROSE: &str = "One line of prose.\n";

/// A throwaway tracker. `std::env::temp_dir` plus pid and a counter rather than a crate —
/// the engine takes no dependencies, and its tests should not either.
struct Tracker(PathBuf);

impl Tracker {
    fn new(tag: &str) -> Tracker {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static N: AtomicUsize = AtomicUsize::new(0);
        let n = N.fetch_add(1, Ordering::Relaxed);
        // The tracker is a *child* of the throwaway root, never the root itself.
        // Discovery treats any directory holding `trck.json` as a tracker and looks one
        // level down from every ancestor — so a bare `trck.json` in the system temp
        // directory turns `/tmp` into somebody else's tracker, and the discovery tests
        // running alongside this one start finding it.
        let root = std::env::temp_dir().join(format!("trck-newbody-{tag}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let dir = root.join("issues");
        std::fs::create_dir_all(dir.join("items")).expect("mkdir");
        std::fs::write(dir.join("trck.json"), "{}\n").expect("config");
        Tracker(root)
    }

    /// The tracker itself, which is not the directory that gets removed.
    fn path(&self) -> PathBuf {
        self.0.join("issues")
    }
}

impl Drop for Tracker {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Run a verb against `tracker`, optionally writing `input` to its stdin.
///
/// `Stdio::null()` when there is no input, which is also what makes this the harness for
/// the no-terminal rule: a closed stdin is exactly what a script has.
fn run(tracker: &Path, args: &[&str], input: Option<&str>) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_trck"))
        .args(args)
        .arg("--dir")
        .arg(tracker)
        .env("NO_COLOR", "1")
        .env("TRCK_NOW", "2026-01-01T00:00:00Z")
        .stdin(if input.is_some() { Stdio::piped() } else { Stdio::null() })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawning trck");
    if let (Some(text), Some(mut pipe)) = (input, child.stdin.take()) {
        pipe.write_all(text.as_bytes()).expect("writing stdin");
    }
    child.wait_with_output().expect("waiting for trck")
}

/// The body file `new` just wrote, whatever its slug turned out to be.
fn body_of(tracker: &Path, id: &str) -> String {
    let items = tracker.join("items");
    let entry = std::fs::read_dir(&items)
        .expect("items")
        .flatten()
        .map(|e| e.path())
        .find(|p| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with(id)))
        .unwrap_or_else(|| panic!("no body file for {id} in {}", items.display()));
    std::fs::read_to_string(entry).expect("body")
}

#[test]
fn a_dash_reads_the_body_from_stdin() {
    let t = Tracker::new("stdin");
    let out = run(&t.path(), &["new", "Alpha", "--id", "aaaaaaa", "--body-file", "-"], Some(PROSE));
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(body_of(&t.path(), "aaaaaaa"), PROSE);
}

/// The acceptance criterion, stated directly: three spellings, one issue.
#[test]
fn the_three_spellings_agree() {
    let inline = Tracker::new("inline");
    assert!(run(&inline.path(), &["new", "Alpha", "--id", "aaaaaaa", "--body", PROSE.trim_end()], None).status.success());

    let piped = Tracker::new("piped");
    assert!(run(&piped.path(), &["new", "Alpha", "--id", "aaaaaaa", "--body-file", "-"], Some(PROSE)).status.success());

    let from_file = Tracker::new("file");
    let prose = from_file.path().join("prose.md");
    std::fs::write(&prose, PROSE).expect("write");
    let spec = prose.display().to_string();
    assert!(run(&from_file.path(), &["new", "Alpha", "--id", "aaaaaaa", "--body-file", &spec], None).status.success());

    let bodies = [body_of(&inline.path(), "aaaaaaa"), body_of(&piped.path(), "aaaaaaa"), body_of(&from_file.path(), "aaaaaaa")];
    assert_eq!(bodies[0], PROSE, "--body");
    assert_eq!(bodies[1], PROSE, "--body-file -");
    assert_eq!(bodies[2], PROSE, "--body-file PATH");

    // And the rows they produced, not just the prose.
    let index = |t: &Tracker| std::fs::read_to_string(t.path().join("index.jsonl")).expect("index");
    assert_eq!(index(&inline), index(&piped));
    assert_eq!(index(&inline), index(&from_file));
}

/// Empty stdin is a body of nothing, not a missing answer: `--body-file -` said where the
/// body comes from, and the answer came back empty.
#[test]
fn empty_stdin_is_an_empty_body_not_a_refusal() {
    let t = Tracker::new("emptystdin");
    let out = run(&t.path(), &["new", "Alpha", "--id", "aaaaaaa", "--body-file", "-"], Some(""));
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(body_of(&t.path(), "aaaaaaa"), "");
}

/// With stdin closed and no flag, nothing is created — asserted on the tracker, not only on
/// the exit code, because "errors but files the issue anyway" is the failure that matters.
#[test]
fn no_flag_with_no_terminal_creates_nothing() {
    let t = Tracker::new("noflag");
    let out = run(&t.path(), &["new", "Alpha", "--id", "aaaaaaa"], None);
    assert!(!out.status.success(), "a body-less new succeeded with no terminal");
    let err = String::from_utf8_lossy(&out.stderr);
    for flag in ["--body", "--body-file", "--empty"] {
        assert!(err.contains(flag), "the refusal must name {flag}: {err}");
    }
    assert_eq!(std::fs::read_to_string(t.path().join("index.jsonl")).unwrap_or_default(), "", "a row was written anyway");
    assert_eq!(std::fs::read_dir(t.path().join("items")).expect("items").count(), 0, "a body was written anyway");
}

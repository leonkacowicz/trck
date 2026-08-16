//! A closed stdout is not an error.
//!
//! `trck list | head` is the most ordinary pipeline anyone will type at this tool, and it
//! ends with the reader gone while the writer still has output. Rust's `println!` unwraps
//! the write, so that used to surface as a panic and a backtrace — the exact outcome the
//! crate's denied `unwrap`/`expect`/`panic` lints exist to prevent, reached through a
//! standard-library macro that walked underneath them.
//!
//! The test closes the read end immediately rather than relying on output exceeding the
//! pipe buffer: with every reader gone, the child's first write fails with `EPIPE` however
//! little it had to say, so this does not depend on the size of a tracker or of a buffer.

// Tests assert; that is their job. The crate denies unwrap/expect/panic because a malformed
// tracker must produce a diagnostic rather than a stack trace, but a test that cannot panic
// cannot fail.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::io::Read as _;
use std::path::PathBuf;
use std::process::{Command, Stdio};

/// A real tracker on disk, with epics, dependencies and enough rows to render.
///
/// The bundled example, not this repository's own: that one lives on the `trck-issues`
/// branch now, and a ref-backed tracker cannot be handed to `--dir` at all. Size is not what
/// this test rests on — see above, the reader is gone before the first write — so the smaller
/// tracker costs it nothing.
///
/// Absent, this fails rather than skipping. It used to return `None` and let the assertions
/// run against empty output — which passes, silently, and would have gone on passing after
/// the crate moved to the repo root had the path been wrong.
fn repo_tracker() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples").join("action-game");
    assert!(dir.join("index.jsonl").is_file(), "no tracker at {}", dir.display());
    dir
}

/// Run a verb with stdout piped, drop the read end at once, and report the child's stderr.
fn stderr_after_closing_stdout(args: &[&str]) -> String {
    let tracker = repo_tracker();
    let mut child = Command::new(env!("CARGO_BIN_EXE_trck"))
        .args(args)
        .arg("--dir")
        .arg(&tracker)
        .env("NO_COLOR", "1")
        .env("TRCK_NOW", "2026-01-01T00:00:00Z")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawning trck");

    // The whole point: the reader goes away before the writer is finished.
    drop(child.stdout.take());

    let mut err = String::new();
    if let Some(mut e) = child.stderr.take() {
        let _ = e.read_to_string(&mut err);
    }
    let _ = child.wait().expect("waiting for trck");
    err
}

#[test]
fn a_closed_stdout_does_not_panic() {
    for verb in [["list", "--all"], ["tree", "--all"], ["deps", "--full"], ["ready", "--next"]] {
        let err = stderr_after_closing_stdout(&verb);
        assert!(!err.contains("panicked"), "`trck {}` panicked on a closed pipe:\n{err}", verb.join(" "));
        assert!(!err.contains("RUST_BACKTRACE"), "`trck {}` offered a backtrace to a user who closed a pipe:\n{err}", verb.join(" "));
    }
}

/// A pipe closing early is the shell working as designed, so it is not the engine's failure
/// to report: nothing should reach stderr at all.
#[test]
fn a_closed_stdout_is_silent() {
    let err = stderr_after_closing_stdout(&["list", "--all"]);
    assert!(err.is_empty(), "noise on stderr after a closed pipe: {err}");
}

//! `which` with no operands reads its paths from stdin.
//!
//! That is the form the verb exists for — `rg -l pattern $(trck list --paths) | trck which` —
//! and it is the one shape the conformance suite cannot state: a fixture is one argv and no
//! standard input, so nothing there can tell a working pipe from a hang. Hence a test that
//! actually spawns the binary and writes to it.

// Tests assert; that is their job. The crate denies unwrap/expect/panic because a malformed
// tracker must produce a diagnostic rather than a stack trace, but a test that cannot panic
// cannot fail.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeSet;
use std::io::Write as _;
use std::path::PathBuf;
use std::process::{Command, Stdio};

/// This repository's own tracker — a real one, with more issues than any fixture.
fn repo_tracker() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("issues");
    assert!(dir.join("index.jsonl").is_file(), "no tracker at {}", dir.display());
    dir
}

/// Run a verb against the repo's tracker, optionally writing `input` to its stdin.
fn run(args: &[&str], input: Option<&str>) -> String {
    let mut child = Command::new(env!("CARGO_BIN_EXE_trck"))
        .args(args)
        .arg("--dir")
        .arg(repo_tracker())
        .env("NO_COLOR", "1")
        .stdin(if input.is_some() { Stdio::piped() } else { Stdio::null() })
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawning trck");
    if let (Some(text), Some(mut pipe)) = (input, child.stdin.take()) {
        pipe.write_all(text.as_bytes()).expect("writing stdin");
    }
    let out = child.wait_with_output().expect("waiting for trck");
    assert!(out.status.success(), "trck {args:?} failed: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8(out.stdout).expect("utf-8 output")
}

/// The id a body filename carries, which is everything before its first `-`.
fn id_of(path: &str) -> String {
    let name = path.rsplit('/').next().unwrap_or(path);
    name.split('-').next().unwrap_or(name).to_string()
}

#[test]
fn paths_piped_on_stdin_resolve_to_their_issues() {
    let listed = run(&["list", "--paths", "--flat", "--all"], None);
    let paths: Vec<&str> = listed.lines().take(3).collect();
    assert_eq!(paths.len(), 3, "the repo tracker should have at least three issues");

    let ids = run(&["which", "--ids"], Some(&(paths.join("\n") + "\n")));

    let got: BTreeSet<String> = ids.lines().map(str::to_string).collect();
    let want: BTreeSet<String> = paths.iter().map(|p| id_of(p)).collect();
    assert_eq!(got, want, "stdin paths did not resolve to their issues");
}

/// A blank line is a formatting artifact of whatever piped in, not a path to look up.
#[test]
fn blank_lines_on_stdin_are_not_looked_up() {
    let listed = run(&["list", "--paths", "--flat", "--all"], None);
    let first = listed.lines().next().expect("at least one issue");

    let ids = run(&["which", "--ids"], Some(&format!("\n{first}\n\n")));

    assert_eq!(ids.lines().collect::<Vec<_>>(), vec![id_of(first)]);
}

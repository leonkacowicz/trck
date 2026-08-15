//! Talking to git: one process wrapper, and the plumbing every git-backed feature calls.
//!
//! There used to be two spawn sites — `diff`'s, for reading a tracker at a revision, and
//! `repo`'s, for writing merge drivers into `.git/config`. Two is one more than a crate
//! needs, and the ref-backed tracker (`#sqzr7nk`) adds a third caller that wants both
//! halves, so they collapse here.
//!
//! The primitives are deliberately thin and deliberately *not* about trackers: they answer
//! "what does this revision hold" and "make this commit", and the layer above decides what
//! that means. This file is only the spawn: [`read`] asks the questions, and the write half —
//! blobs, trees, commits, refs — is in [`write`], because a tracker that only reads never
//! touches it.
//!
//! **Errors are unphrased on purpose.** A failed spawn says `git is not on PATH` and nothing
//! else; `diff` turns that into a sentence about revision specs being unavailable, because
//! only the caller knows which flag the user should reach for instead. A shared wrapper that
//! guessed would make every caller's diagnostic slightly wrong.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};

// Reached as `git::write::hash_object` rather than re-exported flat: the read primitives are
// what most callers want, and the qualifier is a reminder that the other four mutate the
// object store. Nothing calls them yet — the write path (`#jgf9ktx`) is the first consumer —
// which is what the crate-level `dead_code` expectation is for.
mod read;
pub(crate) mod refs;
pub(crate) mod write;

pub(crate) use read::{changed_paths, is_ancestor, ls_tree, repo_root, rev_parse, show, tree_blobs};

/// What a failed spawn says. Callers add the context; see the module note.
const NO_GIT: &str = "git is not on PATH";

/// Run git, and hand back the raw output.
///
/// `env` exists for `GIT_INDEX_FILE`, which is how a tree is built without touching the
/// caller's index, and `stdin` for the commands that take their payload that way —
/// `hash-object --stdin`, `update-index --index-info`, `commit-tree`. Feeding those through
/// arguments instead would put issue bodies and commit messages on a command line, where
/// length limits and leading dashes are someone else's problem.
fn exec(cwd: &Path, args: &[&str], env: &[(&str, &str)], stdin: Option<&[u8]>) -> Result<Output, String> {
    let mut cmd = command(cwd, args, env);
    match stdin {
        None => cmd.output().map_err(|_| NO_GIT.to_string()),
        Some(input) => feed(cmd, input),
    }
}

/// The one place in the crate that builds a [`Command`].
fn command(cwd: &Path, args: &[&str], env: &[(&str, &str)]) -> Command {
    let mut cmd = Command::new("git");
    cmd.args(args).current_dir(cwd);
    for (k, v) in env {
        cmd.env(k, v);
    }
    cmd
}

/// Run a prepared command with `input` on its stdin.
///
/// Taking the pipe handle and dropping it is what tells git the input is complete. Holding
/// it past the write deadlocks: git waits for end-of-file, we wait for git. A write that
/// failed is reported *after* the wait for the same reason — git may have exited early
/// (a bad object, a refused update), and its own diagnostic is the more useful of the two.
fn feed(mut cmd: Command, input: &[u8]) -> Result<Output, String> {
    cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd.spawn().map_err(|_| NO_GIT.to_string())?;
    let mut pipe = child.stdin.take().ok_or_else(|| "git: stdin unavailable".to_string())?;
    let written = pipe.write_all(input).map_err(|e| format!("writing to git: {e}"));
    drop(pipe);
    let out = child.wait_with_output().map_err(|e| format!("running git: {e}"))?;
    written.map(|()| out)
}

/// Run git and hand back the raw [`Output`], exit status included.
///
/// For the callers that treat a non-zero exit as an answer rather than a failure — "does
/// this revision exist", "is this a repository" — rather than as something to report.
pub(crate) fn run(cwd: &Path, args: &[&str]) -> Result<Output, String> {
    exec(cwd, args, &[], None)
}

/// Run git, and treat a non-zero exit as an error naming what was run and what it said.
pub(crate) fn stdout(cwd: &Path, args: &[&str]) -> Result<String, String> {
    let out = exec(cwd, args, &[], None)?;
    if !out.status.success() {
        return Err(format!("git {}: {}", args.join(" "), String::from_utf8_lossy(&out.stderr).trim()));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

#[cfg(test)]
pub(crate) mod tests {
    // Tests assert; that is their job. See the note in `discovery::tests`.
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;
    use crate::discovery::tests::Tmp;
    use std::path::PathBuf;

    /// A throwaway repository with an identity, or `None` where git is not installed.
    ///
    /// `None` rather than a failure, the way `tests/app_js.rs` skips without node: git is
    /// this module's subject, so its absence makes the tests unrunnable rather than failed.
    /// The identity is set locally because `commit-tree` needs one and a machine's global
    /// config is not this test's to assume.
    pub(crate) fn repo(tag: &str) -> Option<(Tmp, PathBuf)> {
        let tmp = Tmp::new(tag);
        let dir = tmp.path().to_path_buf();
        match run(&dir, &["init", "-q", "-b", "main"]) {
            Err(_) => return None,
            Ok(out) if !out.status.success() => return None,
            Ok(_) => {},
        }
        stdout(&dir, &["config", "user.email", "trck@example.invalid"]).expect("set email");
        stdout(&dir, &["config", "user.name", "trck tests"]).expect("set name");
        Some((tmp, dir))
    }

    /// Commit `text` as `name`, answering with the commit sha.
    pub(crate) fn commit_file(dir: &Path, name: &str, text: &str) -> String {
        std::fs::write(dir.join(name), text).expect("write");
        stdout(dir, &["add", name]).expect("add");
        stdout(dir, &["commit", "-q", "-m", "commit"]).expect("commit");
        stdout(dir, &["rev-parse", "HEAD"]).expect("head")
    }

    /// What this file is responsible for: the spawn, and reporting a failure with what ran
    /// and what git said. What was *asked* is [`super::read`]'s and [`super::write`]'s.
    #[test]
    fn a_failing_invocation_is_reported_with_what_ran_and_what_git_said() {
        let Some((_tmp, dir)) = repo("git-failure") else { return };
        let err = stdout(&dir, &["rev-parse", "--verify", "nope"]).expect_err("unknown revision");
        assert!(err.starts_with("git rev-parse --verify nope:"), "{err}");
    }
}

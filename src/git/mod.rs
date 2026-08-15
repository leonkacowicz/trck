//! Talking to git: one process wrapper, and the plumbing every git-backed feature calls.
//!
//! There used to be two spawn sites — `diff`'s, for reading a tracker at a revision, and
//! `repo`'s, for writing merge drivers into `.git/config`. Two is one more than a crate
//! needs, and the ref-backed tracker (`#sqzr7nk`) adds a third caller that wants both
//! halves, so they collapse here.
//!
//! The primitives are deliberately thin and deliberately *not* about trackers: they answer
//! "what does this revision hold" and "make this commit", and the layer above decides what
//! that means. Reads are in this file; the write half — blobs, trees, commits, refs — is in
//! [`write`], because a tracker that only reads never touches it.
//!
//! **Errors are unphrased on purpose.** A failed spawn says `git is not on PATH` and nothing
//! else; `diff` turns that into a sentence about revision specs being unavailable, because
//! only the caller knows which flag the user should reach for instead. A shared wrapper that
//! guessed would make every caller's diagnostic slightly wrong.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

// Reached as `git::write::hash_object` rather than re-exported flat: the read primitives are
// what most callers want, and the qualifier is a reminder that the other four mutate the
// object store. Nothing calls them yet — the write path (`#jgf9ktx`) is the first consumer —
// which is what the crate-level `dead_code` expectation is for.
pub(crate) mod write;

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

/// The commit `rev` names, or `None` when it names nothing.
///
/// `^{commit}` rather than a bare resolve, so a tag or a tree answers with the commit or not
/// at all — a caller asking for a revision wants something it can read a tree out of.
pub(crate) fn rev_parse(cwd: &Path, rev: &str) -> Result<Option<String>, String> {
    let out = run(cwd, &["rev-parse", "--verify", "--quiet", &format!("{rev}^{{commit}}")])?;
    if !out.status.success() {
        return Ok(None);
    }
    Ok(Some(String::from_utf8_lossy(&out.stdout).trim().to_string()))
}

/// The contents of `path` at `rev`, or `None` when the revision does not hold it.
///
/// Untrimmed, unlike [`stdout`]: this is file content, and a tracker's `index.jsonl` is
/// newline-terminated by definition. Absence is `None` rather than an error because it is
/// a legitimate answer — a revision from before the tracker existed holds no index, and
/// that means "everything is new", not "something went wrong".
pub(crate) fn show(cwd: &Path, rev: &str, path: &str) -> Result<Option<String>, String> {
    let out = run(cwd, &["show", &format!("{rev}:{path}")])?;
    if !out.status.success() {
        return Ok(None);
    }
    Ok(Some(String::from_utf8_lossy(&out.stdout).into_owned()))
}

/// The names directly inside `rev:dir`, sorted, or `None` when the revision has no such
/// directory.
///
/// `--name-only` and no `-r`, so this answers with one level the way `read_dir` does rather
/// than the whole subtree. Absence is `None` for the same reason it is in [`show`]: a
/// tracker whose `items/` has not been created yet is empty, not broken.
pub(crate) fn ls_tree(cwd: &Path, rev: &str, dir: &str) -> Result<Option<Vec<String>>, String> {
    let out = run(cwd, &["ls-tree", "--name-only", "-z", &format!("{rev}:{dir}")])?;
    if !out.status.success() {
        return Ok(None);
    }
    // NUL-separated: a name git would otherwise quote and escape comes back verbatim, and
    // an issue slug is not guaranteed to be free of anything git considers unusual.
    let mut names: Vec<String> = String::from_utf8_lossy(&out.stdout).split('\0').filter(|n| !n.is_empty()).map(str::to_string).collect();
    names.sort();
    Ok(Some(names))
}

/// The working tree's root, or `None` when `cwd` is not inside a repository.
pub(crate) fn repo_root(cwd: &Path) -> Result<Option<PathBuf>, String> {
    let out = run(cwd, &["rev-parse", "--show-toplevel"])?;
    if !out.status.success() {
        return Ok(None);
    }
    Ok(Some(PathBuf::from(String::from_utf8_lossy(&out.stdout).trim())))
}

#[cfg(test)]
pub(crate) mod tests {
    // Tests assert; that is their job. See the note in `discovery::tests`.
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;
    use crate::discovery::tests::Tmp;

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

    #[test]
    fn rev_parse_answers_with_the_commit_and_with_none_for_an_unknown_revision() {
        let Some((_tmp, dir)) = repo("git-revparse") else { return };
        let head = commit_file(&dir, "a.txt", "one\n");
        assert_eq!(rev_parse(&dir, "HEAD").unwrap(), Some(head));
        assert_eq!(rev_parse(&dir, "no-such-branch").unwrap(), None);
    }

    #[test]
    fn show_reads_content_untrimmed_and_answers_none_for_a_path_the_revision_lacks() {
        let Some((_tmp, dir)) = repo("git-show") else { return };
        commit_file(&dir, "index.jsonl", "{\"id\": \"a\"}\n");
        assert_eq!(show(&dir, "HEAD", "index.jsonl").unwrap(), Some("{\"id\": \"a\"}\n".to_string()));
        assert_eq!(show(&dir, "HEAD", "items/nope.md").unwrap(), None);
    }

    #[test]
    fn a_failing_invocation_is_reported_with_what_ran_and_what_git_said() {
        let Some((_tmp, dir)) = repo("git-failure") else { return };
        let err = stdout(&dir, &["rev-parse", "--verify", "nope"]).expect_err("unknown revision");
        assert!(err.starts_with("git rev-parse --verify nope:"), "{err}");
    }

    #[test]
    fn repo_root_finds_the_top_level_and_answers_none_outside_a_repository() {
        let Some((tmp, dir)) = repo("git-root") else { return };
        commit_file(&dir, "a.txt", "one\n");
        let nested = dir.join("sub");
        std::fs::create_dir_all(&nested).expect("mkdir");
        let root = repo_root(&nested).unwrap().expect("inside a repo");
        assert_eq!(root.canonicalize().ok(), dir.canonicalize().ok());
        // `Tmp`'s own root is a plain directory: the repository is the one made inside it.
        let outside = tmp.path().parent().expect("temp dir has a parent").to_path_buf();
        if repo_root(&outside).unwrap().is_some() {
            return; // the system temp dir is itself inside a repository; nothing to assert.
        }
        assert_eq!(repo_root(&outside).unwrap(), None);
    }
}

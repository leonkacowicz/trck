//! Talking to git: running it in the tracker, and naming ourselves to it.

use std::path::Path;

/// Run a git command in the repository, returning its trimmed stdout.
pub(super) fn git(cwd: &Path, args: &[&str]) -> Result<String, String> {
    crate::git::stdout(cwd, args)
}

/// Assert we are inside a git repository, answering with the given git query.
///
/// Both git verbs open this way, and both want the same refusal: `git`'s own message names a
/// plumbing command the user did not run, which is not the thing they need to hear.
pub(super) fn require_repo(cwd: &Path, query: &str) -> Result<String, String> {
    if !crate::git::run(cwd, &["rev-parse", "--git-dir"])?.status.success() {
        return Err("not a git repository".to_string());
    }
    git(cwd, &["rev-parse", query])
}

/// How a git driver or hook should re-invoke this engine.
///
/// The absolute path of the running binary, never a bare `trck`. The driver command is
/// baked into `.git/config` and fires much later, from whatever environment git happens to
/// have: a `PATH` lookup need not resolve at all (a CI checkout installs nothing) and, where
/// it does, need not be this engine or this version. An absolute path is answerable now.
///
/// Unlike the Python engine this needs no interpreter prefix — the binary is the artifact —
/// and for the same reason there is no vendored-copy case: a vendored `trck` beside the
/// tracker is a Python script, which this engine cannot claim to be.
pub(super) fn engine_invocation() -> Result<String, String> {
    let exe = std::env::current_exe().map_err(|e| format!("locating this binary: {e}"))?;
    Ok(format!("\"{}\"", exe.display()))
}

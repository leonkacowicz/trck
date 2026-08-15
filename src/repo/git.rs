//! Talking to git: running it in the tracker, and naming ourselves to it.

use crate::discovery::Ctx;

/// Run a git command in the tracker directory, returning its trimmed stdout.
///
/// The tracker directory is the only thing this adds over [`crate::git::stdout`]; the
/// primitives themselves live there, since `diff` and the ref-backed source want them
/// against a path rather than a loaded tracker.
pub(super) fn git(ctx: &Ctx, args: &[&str]) -> Result<String, String> {
    crate::git::stdout(ctx.dir()?, args)
}

/// Assert we are inside a git repository, answering with the given git query.
///
/// Both git verbs open this way, and both want the same refusal: `git`'s own message names a
/// plumbing command the user did not run, which is not the thing they need to hear.
pub(super) fn require_repo(ctx: &Ctx, query: &str) -> Result<String, String> {
    git(ctx, &["rev-parse", query]).map_err(|_| "not a git repository".to_string())
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

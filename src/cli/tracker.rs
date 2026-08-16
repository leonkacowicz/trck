//! How a verb gets the tracker it acts on.
//!
//! Resolution and loading are one step from a verb's point of view and two from the
//! engine's: `--dir` names a directory, `--ref` names a revision, and [`Ctx`] carries
//! whichever it turned out to be. Keeping the pair here means `mod.rs` stays a description
//! of the command line rather than a description of storage.

use super::Args;
use crate::discovery::{Ctx, Source};

/// Which tracker the invocation means, without loading it.
pub(super) fn tracker_source(args: &Args) -> Result<Source, String> {
    let cwd = std::env::current_dir().map_err(|e| format!("cannot read the working directory: {e}"))?;
    let env = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
    let (env_dir, env_ref) = (env("TRCK_DIR"), env("TRCK_REF"));
    let over = crate::discovery::Overrides { dir: args.opt("--dir"), env_dir: env_dir.as_deref(), git_ref: args.opt("--ref"), env_ref: env_ref.as_deref() };
    crate::discovery::resolve_tracker_source(&over, &cwd)
}

/// Where the tracker is on disk. Split out for `migrate-layout`, which must reach a tracker
/// the guards in `Ctx::load` would refuse — and which, being a filesystem migration, has
/// nothing to do for a tracker that is not on a filesystem.
pub(super) fn tracker_dir(args: &Args) -> Result<std::path::PathBuf, String> {
    match tracker_source(args)? {
        Source::Dir(dir) => Ok(dir),
        Source::Ref { rev, .. } => Err(format!("the tracker is git ref '{rev}', which has no files on disk")),
    }
}

pub(super) fn context(args: &Args) -> Result<Ctx, String> {
    Ctx::load(tracker_source(args)?, true)
}

/// The invocation directory and any tracker `setup-git` can resolve.
///
/// Clone-local configuration is also the remedy for an implicitly hidden tracker ref, so
/// absence is allowed there. Explicit overrides stay strict: silently ignoring a mistyped
/// `--dir` or `--ref` would configure the wrong repository.
pub(super) fn setup_source(args: &Args) -> Result<(std::path::PathBuf, Option<Ctx>), String> {
    let cwd = std::env::current_dir().map_err(|e| format!("cannot read the working directory: {e}"))?;
    let env_set = |key: &str| std::env::var(key).is_ok_and(|value| !value.is_empty());
    let explicit = [args.opt("--dir").is_some(), args.opt("--ref").is_some(), env_set("TRCK_DIR"), env_set("TRCK_REF")].contains(&true);
    match tracker_source(args) {
        Ok(source) => Ok((cwd, Some(Ctx::load(source, true)?))),
        Err(error) => hidden_setup_source(cwd, error, explicit),
    }
}

fn hidden_setup_source(cwd: std::path::PathBuf, error: String, explicit: bool) -> Result<(std::path::PathBuf, Option<Ctx>), String> {
    (!explicit)
        .then(|| crate::discovery::refspec::why_invisible(&cwd, crate::discovery::TRACKER_REF))
        .flatten()
        .filter(|hidden| hidden == &error)
        .map(|_| (cwd, None))
        .ok_or(error)
}

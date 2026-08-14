//! How a verb gets the tracker it acts on.
//!
//! Resolution and loading are one step from a verb's point of view and two from the
//! engine's: `--dir` names a directory, `--ref` names a revision, and only the first is
//! something `Ctx::load` can open today. Keeping the pair here means `mod.rs` stays a
//! description of the command line rather than a description of storage.

use super::Args;
use crate::discovery::Ctx;

/// Which tracker the invocation means, without loading it.
pub(super) fn tracker_source(args: &Args) -> Result<crate::discovery::Source, String> {
    let cwd = std::env::current_dir().map_err(|e| format!("cannot read the working directory: {e}"))?;
    let env = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
    let (env_dir, env_ref) = (env("TRCK_DIR"), env("TRCK_REF"));
    let over = crate::discovery::Overrides { dir: args.opt("--dir"), env_dir: env_dir.as_deref(), git_ref: args.opt("--ref"), env_ref: env_ref.as_deref() };
    crate::discovery::resolve_tracker_source(&over, &cwd)
}

/// Where the tracker is on disk. Split out for `migrate-layout`, which must reach a tracker
/// the guards in `Ctx::load` would refuse.
pub(super) fn tracker_dir(args: &Args) -> Result<std::path::PathBuf, String> {
    match tracker_source(args)? {
        crate::discovery::Source::Dir(dir) => Ok(dir),
        crate::discovery::Source::Ref(r) => Err(unreadable_ref(&r)),
    }
}

pub(super) fn context(args: &Args) -> Result<Ctx, String> {
    Ctx::load(tracker_dir(args)?, true)
}

/// What a resolved-but-unreadable ref says.
///
/// Resolution and reading land separately, so between them a checkout with a tracker branch
/// and no `issues/` directory resolves to something this engine cannot open. Saying which
/// ref it found, and naming the flag that reaches a directory instead, is the difference
/// between a dead end and a next step.
fn unreadable_ref(r: &str) -> String {
    format!("tracker resolved to git ref '{r}', which this engine cannot read; pass --dir to act on a directory instead")
}

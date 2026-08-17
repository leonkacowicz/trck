//! Routing for `repo`, the tracker-maintenance verb and its subcommands.
//!
//! Its own file, mirroring `help/maintain.rs`, because `dispatch`'s three groups are the verbs
//! that change an issue, the verbs that read one, and this — and `repo` is the only one of the
//! three whose subcommands each need the context resolved differently, which is a page of
//! reasons that belongs beside the code they explain rather than under a fourth heading in a
//! file about routing.

use super::{Args, context, setup_source, tracker_dir};
use crate::discovery::Ctx;
use crate::repo;

/// `repo` and its subcommands.
///
/// The context is resolved per subcommand rather than once, because they disagree about
/// what they need: the merge drivers must work with no tracker in reach at all, and
/// `migrate-layout` must reach one the ordinary guards would refuse.
pub(super) fn dispatch_repo(args: &Args) -> Result<String, String> {
    let sub = args.positional_at(0).unwrap_or("");
    let operand = |n: usize| -> Result<&str, String> { args.positional_at(n).ok_or_else(|| format!("repo {sub}: missing operand {n}")) };
    match sub {
        // git may invoke a driver from anywhere in the worktree, and a merge with no
        // reachable trck.json still has to merge the rows it was handed.
        "merge-index" => repo::cmd_merge_index(context(args).ok().as_ref(), operand(1)?, operand(2)?, operand(3)?),
        "merge-summary" => repo::cmd_merge_summary(context(args).ok().as_ref(), operand(1)?),
        // Clone-local setup is also how an implicitly hidden tracker ref becomes visible.
        "setup-git" => {
            let (cwd, context) = setup_source(args)?;
            repo::cmd_setup_git(&cwd, context.as_ref())
        },
        "install-hook" => repo::cmd_install_hook(&context(args)?),
        "normalize" => repo::cmd_normalize(&context(args)?),
        // The one verb whose whole job is to operate on a legacy tracker, so it resolves
        // the context without the layout guard that refuses one.
        "migrate-layout" => repo::cmd_migrate_layout(&Ctx::load(crate::discovery::Source::Dir(tracker_dir(args)?), false)?, args.has("--dry-run")),
        "" => Err("repo: missing a subcommand".into()),
        other => Err(format!("repo: `{other}` is not implemented yet in the Rust engine")),
    }
}

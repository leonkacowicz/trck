//! The `repo` verbs: git integration and one-shot tracker maintenance.
//!
//! These differ in kind from the read and write verbs. Those operate on a settled tracker;
//! these write into `.git`, and the [`drivers`] **run inside a merge**, where the working tree
//! is not yet the merged result.
//!
//! One module per verb — [`setup`], [`hook`], [`migrate`], [`drivers`] — plus the two things
//! more than one of them needs: [`git`] for talking to git at all, and [`attributes`] for the
//! committed half of `setup-git`. `normalize` stays here; it is four lines and touches neither
//! git nor the layout.

use crate::discovery::Ctx;
use crate::index::parse_index;

mod attributes;
mod drivers;
mod git;
mod hook;
mod migrate;
mod setup;

pub(crate) use drivers::{cmd_merge_index, cmd_merge_summary};
pub(crate) use hook::cmd_install_hook;
pub(crate) use migrate::cmd_migrate_layout;
pub(crate) use setup::cmd_setup_git;

/// `repo normalize` — rewrite `index.jsonl` in canonical slim form.
///
/// No data change: it re-serialises through the same write path every verb ends in, which
/// also regenerates the summary and re-derives what is derived.
pub(crate) fn cmd_normalize(ctx: &Ctx) -> Result<String, String> {
    let rows = parse_index(&ctx.read_index()?, "index.jsonl")?;
    let n = rows.len();
    crate::verbs::commit(ctx, rows, Vec::new(), &crate::verbs::Op::new("normalize"))?;
    Ok(format!("normalized {} ({n} issues)", ctx.index_path()?.display()))
}

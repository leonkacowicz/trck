//! `edit`: rewrite an existing issue's prose.
//!
//! Today a body is edited by opening `issues/items/<id>-<slug>.md`. Once the tracker lives on
//! a ref that file is not on disk at all, so without this verb body edits stop being possible
//! rather than merely becoming different — which is why it blocks the flip rather than being
//! one more convenience.
//!
//! Mechanically it is `new`'s editor path with different starting content: the current body
//! instead of the template. Everything downstream — the changeset, the backend, the commit —
//! is the same, so a ref-backed tracker gets this for free.

use super::Args;
use super::body;
use crate::discovery::Ctx;
use crate::verbs::{Edit, Op, body_rel_path, commit, load_rows, resolve_ref};

pub(super) fn cmd_edit(ctx: &Ctx, args: &Args) -> Result<String, String> {
    let token = args.positional_at(0).ok_or_else(|| "edit: missing an issue id".to_string())?;
    let rows = load_rows(ctx)?;
    // The same resolution every other verb uses, so an unknown id reads the same here.
    let iid = resolve_ref(&rows, token)?;
    let row = rows.iter().find(|r| r.id == iid).ok_or_else(|| format!("no issue matching '{iid}'"))?.clone();

    let was = ctx.read_body(&row)?;
    let spec = body::body_spec("edit", args)?;
    let Some(now) = body::resolve_seeded(&spec, &body::Prose { verb: "edit", title: &row.title, interactive: body::interactive(), seed: &was })? else {
        return Ok(format!("#{iid} unchanged"));
    };
    // A body flag can hand back exactly what was already there. Committing that would push a
    // commit whose diff is empty, which on a shared tracker branch is noise everyone else
    // has to rebase over.
    if now == was {
        return Ok(format!("#{iid} unchanged"));
    }

    commit(ctx, rows, vec![Edit::Write { path: body_rel_path(&row), contents: now }], &Op::new("edit").operand(&iid))?;
    Ok(format!("#{iid} edited"))
}

/// The spec `edit` accepts, which is `new`'s minus the ones that describe a *new* row.
pub(super) const EDIT_FLAGS: &[&str] = &["--body", "--body-file", "--empty"];

/// The two verbs that take prose.
///
/// Split from the rest of the write side because they answer a different question: not what
/// to change about a row, but where its words come from — and both reach the same flags,
/// the same editor and the same abort rules to answer it.
pub(super) fn dispatch_prose(args: &Args) -> Option<Result<String, String>> {
    Some(match args.verb.as_str() {
        "new" => super::context(args).and_then(|c| super::opts::new_opts(args).and_then(|o| crate::verbs::cmd_new(&c, &o))),
        "edit" => super::context(args).and_then(|c| cmd_edit(&c, args)),
        _ => return None,
    })
}

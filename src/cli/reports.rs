//! The read verbs that produce a *report* rather than a listing: `diff`, `changelog` and
//! `check`.
//!
//! They sit apart from `dispatch` because they are the verbs it does not merely route: each
//! is a few lines of orchestration over `diff`/`validate`, and a dispatch file that also
//! implements three verbs is a dispatch file nobody can skim.

use super::{Args, emit, is_closed_pipe};
use crate::discovery::Ctx;
use crate::verbs;

/// What shipped since a cutoff.
pub(super) fn cmd_changelog(ctx: &Ctx, args: &Args) -> Result<String, String> {
    // `usage_error` has already established --since is present.
    let since = crate::diff::parse_since(args.opt("--since").unwrap_or_default())?;
    let rows = verbs::load_rows(ctx)?;
    let shipped = crate::diff::select_shipped(&rows, &since);
    Ok(crate::diff::render_changelog(&shipped, &since).trim_end_matches('\n').to_string())
}

/// Compare the tracker at two points and report what changed.
///
/// A bare revision spec goes through git; `--from`/`--to` name sources directly and never
/// touch it. With neither, the default is HEAD versus the working tree — "what have I not
/// committed?" — which is the git path too.
///
/// The output is deliberately minimal, one plain line per changed issue. The real layouts
/// are separate issues in the Python engine and are not ported ahead of it: see #2w5panf.
pub(super) fn cmd_diff(ctx: &Ctx, args: &Args) -> Result<String, String> {
    let (old, new) = if let Some(rev) = args.positional_at(0) {
        let (old_rev, new_rev) = crate::diff::revisions::parse_rev_spec(rev)?;
        let old = crate::diff::revisions::git_snapshot(ctx, &old_rev)?;
        let new = match new_rev {
            Some(r) => crate::diff::revisions::git_snapshot(ctx, &r)?,
            None => crate::diff::revisions::resolve_source(args.opt("--to"), ctx)?,
        };
        (old, new)
    } else if let Some(from) = args.opt("--from") {
        (crate::diff::revisions::resolve_source(Some(from), ctx)?, crate::diff::revisions::resolve_source(args.opt("--to"), ctx)?)
    } else {
        (crate::diff::revisions::git_snapshot(ctx, "HEAD")?, crate::diff::revisions::resolve_source(args.opt("--to"), ctx)?)
    };
    let changes = crate::diff::diff_snapshots(&old, &new);
    let mut out = vec![format!("{} → {}", old.label, new.label)];
    if changes.is_empty() {
        out.push("no changes".into());
        return Ok(out.join("\n"));
    }
    for c in &changes {
        let sigil = match c.kind {
            "added" => "+",
            "removed" => "-",
            _ => "~",
        };
        let detail = match c.kind {
            "added" => "new".to_string(),
            "removed" => "removed".to_string(),
            _ => crate::diff::change_summary(c),
        };
        out.push(format!("{sigil} #{} {detail} — {}", c.id, c.title));
    }
    Ok(out.join("\n"))
}

/// `check` prints its findings on stdout and fails on any error. The report *is* the
/// message, so a failing run returns an empty error rather than a second diagnostic.
pub(super) fn cmd_check(ctx: &Ctx) -> Result<String, String> {
    let rows = verbs::load_rows(ctx)?;
    let report = crate::validate::validate(ctx, &rows)?;
    let mut out: Vec<String> = report.warnings.iter().map(|w| format!("warning: {w}")).collect();
    out.extend(report.errors.iter().map(|e| format!("error: {e}")));
    if report.errors.is_empty() {
        out.push(format!("OK — {} issues, 0 errors, {} warning(s)", rows.len(), report.warnings.len()));
        return Ok(out.join("\n"));
    }
    out.push(String::new());
    out.push(format!("{} error(s), {} warning(s) — FAIL", report.errors.len(), report.warnings.len()));
    // Printed here rather than returned because this path exits non-zero with its report on
    // *stdout*, which the `Err` arm of `main` cannot express. A reader that has gone away
    // still must not turn a failed check into a panic.
    if let Err(ref e) = emit(&(out.join("\n") + "\n")) {
        if !is_closed_pipe(e) {
            eprintln!("error: writing output: {e}");
        }
    }
    Err(String::new())
}

/// `summary`: regenerate the committed rollup.
///
/// It writes — except when there is nowhere to write to. A ref-backed tracker has no file
/// beside its index, and refusing there would make the one verb whose entire output *is*
/// the rollup the one verb that cannot show it. So it prints instead: same bytes, different
/// destination.
pub(crate) fn cmd_summary(ctx: &Ctx) -> Result<String, String> {
    let g = crate::graph::Graph::new(verbs::load_rows(ctx)?);
    if ctx.dir().is_err() {
        return Ok(crate::summary::generate_summary(&g));
    }
    let n = g.rows.len();
    verbs::write_summary(ctx, &g)?;
    Ok(format!("wrote {} ({n} issues)", ctx.summary_path()?.display()))
}

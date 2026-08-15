//! The git merge drivers for `index.jsonl` and `SUMMARY.md`.
//!
//! These run **inside a merge**, where the working tree is not yet the merged result. So the
//! contract is not "produces the right file" but "produces the right file given three inputs
//! git hands it mid-operation", and the behaviour under conflict is part of it.

use crate::discovery::Ctx;
use crate::graph::Graph;
use crate::index::{parse_index, render_index};
use crate::issue::Issue;
use crate::json::Json;
use crate::merge::{conflict_ids, merge_rows};
use crate::summary::generate_summary;
use crate::verbs::write_atomic;
use std::collections::BTreeSet;
use std::path::Path;

/// `repo merge-index BASE CURRENT OTHER` — the git merge driver for `index.jsonl`.
///
/// Git passes `%O` (common ancestor), `%A`, `%B` and takes the contents of `%A` as the
/// result; exit 0 means resolved, non-zero conflicted.
///
/// On a clean merge this also regenerates `SUMMARY.md` **from the merged rows**, not by
/// re-reading the working-tree index, which during a merge is not yet the merged result.
/// That is what makes the driver-ordering question moot: git gives no ordering guarantee
/// between per-file drivers, so whichever runs first, the rollup derives from the same rows.
///
/// On a conflict it writes marker blocks and leaves `SUMMARY.md` alone. A rollup regenerated
/// from a half-merged index would launder the conflict into a plausible-looking file; a
/// stale rollup is obvious, a fabricated one is not.
pub(crate) fn cmd_merge_index(ctx: Option<&Ctx>, base: &str, current: &str, other: &str) -> Result<String, String> {
    let (base_rows, a_rows, b_rows) = (read_rows(base)?, read_rows(current)?, read_rows(other)?);
    let (rows, conflicts) = merge_rows(&base_rows, &a_rows, &b_rows)?;
    let dest = Path::new(current);

    if conflicts.is_empty() {
        write_atomic(dest, &render_index(&rows))?;
        if let Some(ctx) = ctx {
            let g = Graph::new(rows);
            write_atomic(&ctx.summary_path()?, &generate_summary(&g))?;
        }
        return Ok(String::new());
    }

    let bad = conflict_ids(&conflicts);
    write_atomic(dest, &marked_up(&rows, &bad, &a_rows, &b_rows))?;
    Err(report(&conflicts))
}

/// Read an index file into raw JSON rows.
///
/// A merge operand is not validated on the way in: it may hold a row this engine would
/// reject, and the merge still has to describe the disagreement rather than die on the
/// parse. A missing file is an empty side — git passes `/dev/null`-like empties for a
/// creation on one branch.
fn read_rows(path: &str) -> Result<Vec<Json>, String> {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let mut out = Vec::new();
    for (n, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        out.push(crate::json::parse(line).map_err(|e| format!("{path} line {}: {e}", n + 1))?);
    }
    Ok(out)
}

/// The conflicted file: the clean rows, then a marker block per conflicting id, so it cannot
/// be parsed — and therefore cannot be `git add`ed unread — until a human resolves it.
///
/// Sides are labelled by position, never by ownership: "ours" and "theirs" swap between a
/// merge and a rebase, so a file that named them would be wrong half the time.
fn marked_up(rows: &[Issue], bad: &BTreeSet<String>, a_rows: &[Json], b_rows: &[Json]) -> String {
    let by = |rows: &[Json], id: &str| -> Option<Json> { rows.iter().find(|r| r.get("id").and_then(Json::as_str) == Some(id)).cloned() };
    let mut out: Vec<String> = rows.iter().filter(|r| !bad.contains(&r.id)).map(|r| r.to_canonical().to_json()).collect();
    for iid in bad {
        out.push(format!("<<<<<<< one side ({iid})"));
        if let Some(r) = by(a_rows, iid) {
            out.push(r.to_json());
        }
        out.push("=======".into());
        if let Some(r) = by(b_rows, iid) {
            out.push(r.to_json());
        }
        out.push(format!(">>>>>>> the other side ({iid})"));
    }
    out.join("\n") + "\n"
}

/// Self-labelled, so `main` prints it verbatim: git shows a driver's stderr to the user
/// as-is, and this is a whole report — headline, the conflicts, then what to do next.
fn report(conflicts: &[String]) -> String {
    let mut lines = vec![format!("trck: index.jsonl has {} unresolved conflict(s):", conflicts.len())];
    lines.extend(conflicts.iter().map(|c| format!("  {c}")));
    lines.push("resolve the marked rows, then `git add` and re-run `trck check`.".into());
    lines.join("\n")
}

/// `repo merge-summary` — discard both sides and regenerate.
///
/// The rollup derives entirely from `index.jsonl`, so there is never anything to merge. This
/// is a safety net rather than the authority: `merge-index` already rewrites `SUMMARY.md`
/// from the rows it merged, which is what makes the order git runs the drivers in irrelevant.
pub(crate) fn cmd_merge_summary(ctx: Option<&Ctx>, current: &str) -> Result<String, String> {
    let Some(ctx) = ctx else {
        // No tracker in reach: leave the file alone rather than truncate it. Reporting
        // success here would tell git a merge resolved when nothing was written.
        return Err("merge-summary: no tracker found to regenerate from".into());
    };
    let text = std::fs::read_to_string(ctx.index_path()?).unwrap_or_default();
    let rows = parse_index(&text, "index.jsonl")?;
    let g = Graph::new(rows);
    write_atomic(Path::new(current), &generate_summary(&g))?;
    Ok(String::new())
}

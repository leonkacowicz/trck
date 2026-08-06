//! The file-path verbs: `path`, `which`, and the `list --paths` renderer they share.
//!
//! trck has no `search`/`grep` of its own, because issue bodies are plain markdown and the
//! search tool is already on the machine. What it owes that tool instead is a way in and a
//! way back: `list --paths`/`path` name the files, `which` names the issues those files are.
//! All three live here so the two directions cannot disagree about what a body file is.

use crate::discovery::Ctx;
use crate::graph::Graph;
use crate::render::{Annotation, RowOpts, render_rows, unique_prefix_lens};
use crate::verbs::{issue_path, load_rows, resolve_ref};
use std::collections::BTreeSet;

/// `path`: one issue's body file.
///
/// The single-issue form of `list --paths`, and it shares that renderer rather than
/// formatting a path of its own — `$(trck path NNN)` and `$(trck list --paths)` name the
/// same file, so they must spell it the same way.
pub(crate) fn cmd_path(ctx: &Ctx, token: &str) -> Result<String, String> {
    let rows = load_rows(ctx)?;
    let iid = resolve_ref(&rows, token)?;
    let g = Graph::new(rows);
    Ok(paths_of(ctx, &g, std::slice::from_ref(&iid)))
}

/// `which`: issue file paths back to issues — the inverse of `path`/`list --paths`.
///
/// Rows come out in tracker order rather than the order the paths arrived in. The input is
/// whatever a grep printed, and its order is the search tool's business; this verb's job is
/// to say *which issues* those files are, and it answers in the order `list` would.
pub(crate) fn cmd_which(ctx: &Ctx, paths: &[String], ids_only: bool) -> Result<String, String> {
    let rows = load_rows(ctx)?;
    let abbrev = unique_prefix_lens(rows.iter().map(|r| r.id.as_str()));
    let g = Graph::new(rows);
    let ids = ids_at_paths(ctx, &g, paths);
    if ids_only {
        return Ok(ids.join("\n"));
    }
    let rows: Vec<&crate::issue::Issue> = ids.iter().filter_map(|id| g.get(id)).collect();
    let row_opts = RowOpts {
        prefix: None,
        dim: &[],
        on_screen: ids.clone(),
        annotate: Annotation::Blocking,
        progress: true,
        show_fields: Vec::new(),
        abbrev: Some(abbrev),
    };
    Ok(render_rows(&g, &rows, &row_opts).join("\n"))
}

/// The paths `which` was handed: its operands, or stdin when it was given none.
///
/// Stdin is the pipeline form — `rg -l pattern $(trck list --paths) | trck which` — and one
/// path per line is what every search tool prints, so the split is on `\n` and nothing else.
/// Blank lines are dropped rather than looked up: a trailing newline is not a request.
pub(crate) fn which_operands(positional: &[String]) -> Result<Vec<String>, String> {
    if !positional.is_empty() {
        return Ok(positional.to_vec());
    }
    let mut text = String::new();
    std::io::Read::read_to_string(&mut std::io::stdin(), &mut text).map_err(|e| format!("stdin: {e}"))?;
    Ok(text.lines().map(str::trim).filter(|l| !l.is_empty()).map(str::to_string).collect())
}

/// The issues those paths name, in tracker order and without repeats.
///
/// Matched on the file name, not the whole path: `list --paths` hands out absolute paths but
/// `rg -l pattern issues/items/*.md` hands out relative ones, and both name the same issue.
/// Anything that is not a body file in this tracker is dropped in silence — the input is
/// whatever a search printed, and it may perfectly well contain other files.
fn ids_at_paths(ctx: &Ctx, g: &Graph, paths: &[String]) -> Vec<String> {
    let named: BTreeSet<&std::ffi::OsStr> = paths.iter().filter_map(|p| std::path::Path::new(p).file_name()).collect();
    g.rows.iter().filter(|r| issue_path(ctx, r).file_name().is_some_and(|n| named.contains(n))).map(|r| r.id.clone()).collect()
}

/// Absolute body paths, one per line — what `--paths` prints for piping into an editor.
pub(super) fn paths_of(ctx: &Ctx, g: &Graph, ids: &[String]) -> String {
    ids.iter()
        .filter_map(|id| g.get(id))
        .map(|r| {
            let p = issue_path(ctx, r);
            p.canonicalize().unwrap_or(p).display().to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

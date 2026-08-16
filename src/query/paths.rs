//! The file-path verbs: `path`, `which`, and the `list --paths` renderer they share.
//!
//! A way in and a way back: `list --paths`/`path` name the files, `which` names the issues
//! those files are. All three live here so the two directions cannot disagree about what a
//! body file is.
//!
//! These used to be how body search was done — issue bodies are plain markdown and the
//! search tool is already on the machine, so `rg -l PATTERN $(trck list --paths) | trck
//! which` was the whole answer. A ref-backed tracker has no files, which falsified the
//! premise rather than the recipe, and body search is [`super::filter`]'s `--contains` now.
//! What is left here is handing a *path*-shaped tool a way to name issues, which is still
//! worth having wherever the tracker is a directory.

use crate::discovery::Ctx;
use crate::graph::Graph;
use crate::render::{Annotation, RowOpts, render_rows, unique_prefix_lens};
use crate::verbs::{load_rows, resolve_ref};
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
    paths_of(ctx, &g, std::slice::from_ref(&iid))
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
    let ids = ids_at_paths(ctx, &g, paths)?;
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
fn ids_at_paths(ctx: &Ctx, g: &Graph, paths: &[String]) -> Result<Vec<String>, String> {
    let named: BTreeSet<&std::ffi::OsStr> = paths.iter().filter_map(|p| std::path::Path::new(p).file_name()).collect();
    let items = ctx.items_dir()?;
    Ok(g.rows.iter().filter(|r| items.join(crate::summary::filename(r)).file_name().is_some_and(|n| named.contains(n))).map(|r| r.id.clone()).collect())
}

/// Absolute body paths, one per line — what `--paths` prints for piping into an editor.
pub(super) fn paths_of(ctx: &Ctx, g: &Graph, ids: &[String]) -> Result<String, String> {
    // Resolved once rather than per row: `items_dir` is the same answer every time, and
    // asking it here is what refuses a ref-backed tracker before any path is printed.
    let items = ctx.items_dir()?;
    Ok(ids
        .iter()
        .filter_map(|id| g.get(id))
        .map(|r| {
            let p = items.join(crate::summary::filename(r));
            plain(&p.canonicalize().unwrap_or(p).display().to_string())
        })
        .collect::<Vec<_>>()
        .join("\n"))
}

/// Windows' longest path, terminator included — the limit the verbatim prefix exists to lift.
const MAX_PATH: usize = 260;

/// Drop a `\\?\` prefix where the plain spelling names the same file, and keep it where it
/// does not.
///
/// `canonicalize` answers in the verbatim form on Windows, so `list --paths` and `path` print
/// `\\?\C:\repo\issues\items\...` — correct, and nothing a user expects to see. But the prefix
/// is load-bearing: a verbatim path reaches the filesystem unnormalized, so `/`, `.`, `..`, a
/// trailing dot or space and the reserved device names are ordinary characters there and are
/// not without it. It is also what lifts `MAX_PATH`. Any of those and the prefix stays — a path
/// that reads better but names nothing is worse than an ugly one that works.
///
/// Pure string work, deliberately: it asks the platform nothing, so the Windows shapes are
/// exercised by the tests below on whatever machine runs them.
fn plain(path: &str) -> String {
    let Some(rest) = path.strip_prefix(r"\\?\") else { return path.to_string() };
    let mut c = rest.chars();
    let is_drive = matches!((c.next(), c.next(), c.next()), (Some(d), Some(':'), Some('\\')) if d.is_ascii_alphabetic());
    // `skip(1)` steps over the drive itself, which is the one component that is meant to
    // carry a colon.
    if !is_drive || rest.len() >= MAX_PATH || rest.contains('/') || rest.split('\\').skip(1).any(needs_verbatim) {
        return path.to_string();
    }
    rest.to_string()
}

/// A component whose meaning the plain spelling would change — or refuse.
///
/// The device names are reserved by their stem, so `con.md` is as unusable as `con`.
fn needs_verbatim(part: &str) -> bool {
    const RESERVED: &[&str] = &["con", "prn", "aux", "nul"];
    if part.is_empty() || part == "." || part == ".." || part.ends_with('.') || part.ends_with(' ') {
        return true;
    }
    let stem = part.split('.').next().unwrap_or(part).to_ascii_lowercase();
    RESERVED.contains(&stem.as_str())
        || matches!(stem.strip_prefix("com").or_else(|| stem.strip_prefix("lpt")), Some(n) if n.len() == 1 && n.starts_with(|c: char| c.is_ascii_digit()))
}

#[cfg(test)]
mod tests {
    // Tests assert; that is their job. The crate denies unwrap/expect/panic because a
    // malformed tracker must produce a diagnostic rather than a stack trace, but a test
    // that cannot panic cannot fail.
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    #[test]
    fn a_path_with_no_verbatim_prefix_is_untouched() {
        assert_eq!(plain("/repo/issues/items/aaa-x.md"), "/repo/issues/items/aaa-x.md");
        assert_eq!(plain(r"C:\repo\issues\items\aaa-x.md"), r"C:\repo\issues\items\aaa-x.md");
    }

    #[test]
    fn an_ordinary_drive_path_loses_the_prefix() {
        assert_eq!(plain(r"\\?\C:\repo\issues\items\aaa-x.md"), r"C:\repo\issues\items\aaa-x.md");
        assert_eq!(plain(r"\\?\d:\repo\x.md"), r"d:\repo\x.md");
    }

    /// `\\?\UNC\server\share` would have to become `\\server\share` — a different rewrite,
    /// and one nothing here produces.
    #[test]
    fn a_unc_share_keeps_its_prefix() {
        assert_eq!(plain(r"\\?\UNC\server\share\x.md"), r"\\?\UNC\server\share\x.md");
    }

    /// The prefix is what lets a path exceed `MAX_PATH`. Strip it there and the result names
    /// a file the API would refuse.
    #[test]
    fn a_path_over_max_path_keeps_its_prefix() {
        let long = format!(r"\\?\C:\{}\x.md", "d".repeat(300));
        assert_eq!(plain(&long), long);
    }

    /// A verbatim path is not normalized: `/`, `.` and `..` are ordinary characters there,
    /// so dropping the prefix would change which file is named.
    #[test]
    fn a_component_the_plain_form_would_reinterpret_keeps_the_prefix() {
        for p in [r"\\?\C:\repo/issues\x.md", r"\\?\C:\repo\.\x.md", r"\\?\C:\repo\..\x.md"] {
            assert_eq!(plain(p), p, "{p}");
        }
    }

    /// Reserved device names, and components ending in a dot or a space: legal verbatim,
    /// reinterpreted or rejected without the prefix.
    #[test]
    fn a_component_windows_reserves_keeps_the_prefix() {
        for p in [r"\\?\C:\repo\NUL\x.md", r"\\?\C:\repo\con.md", r"\\?\C:\repo\COM1\x.md", r"\\?\C:\repo\odd.\x.md", r"\\?\C:\repo\odd \x.md"] {
            assert_eq!(plain(p), p, "{p}");
        }
    }

    #[test]
    fn something_that_is_not_a_drive_path_keeps_the_prefix() {
        assert_eq!(plain(r"\\?\x.md"), r"\\?\x.md");
        assert_eq!(plain(r"\\?\C\repo\x.md"), r"\\?\C\repo\x.md");
    }
}

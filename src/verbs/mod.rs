//! The mutating verbs, and the write path every one of them ends in.
//!
//! Two things are shared by all of them and worth reading first.
//!
//! [`finalize`] is the single write path: derive what is derived, write the index, write
//! the summary. Deriving on write rather than in each verb is what makes the rollup
//! uniform across `mv`, `start`, `done`, `new --parent` and re-parenting with no
//! per-command hooks.
//!
//! Writes are **atomic**: a temporary file in the same directory, then a rename. An
//! interrupted run leaves the previous index intact rather than half a line, which
//! matters because the index is the tracker's only source of truth.

mod clock;
mod edit;

pub(crate) use clock::now_utc;
pub(crate) use edit::{MvOpts, NewOpts, SetOpts, cmd_dep, cmd_label, cmd_mv, cmd_new, cmd_set};

use crate::config::{self, is_terminal};
use crate::discovery::Ctx;
use crate::graph::Graph;
use crate::index::{parse_index, render_index};
use crate::issue::{DEFAULT_POINTS, Issue};
use crate::summary::{filename, generate_summary};
use std::path::{Path, PathBuf};

/// The prose skeleton a new issue's body starts from.
pub(super) const TEMPLATE: &str = "# {title}\n\
    \n\
    ## Summary\n\
    <!-- What needs doing and why. For an epic, link the spec instead of re-narrating it. -->\n\
    \n\
    ## Acceptance criteria\n\
    - [ ]\n\
    \n\
    ## Notes\n\
    <!-- Context, links to files/commits, open questions, decisions. -->\n";

/// A filesystem-safe slug from a title: lowercase, runs of non-alphanumerics collapsed
/// to a single dash, trimmed.
pub(crate) fn slugify(text: &str) -> String {
    let mut out = String::new();
    let mut pending_dash = false;
    for c in text.chars() {
        if c.is_ascii_alphanumeric() {
            if pending_dash && !out.is_empty() {
                out.push('-');
            }
            pending_dash = false;
            out.push(c.to_ascii_lowercase());
        } else if c.is_ascii() || !c.is_alphanumeric() {
            pending_dash = true;
        } else {
            // Non-ASCII alphanumerics are not filesystem-safe across platforms and
            // Python's slugify drops them too.
            pending_dash = true;
        }
    }
    out
}

/// Whether a slug is usable as a filename component.
pub(crate) fn check_slug(slug: &str) -> bool {
    let mut chars = slug.chars();
    chars.next().is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit()) && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

pub(crate) fn issue_path(ctx: &Ctx, row: &Issue) -> PathBuf {
    ctx.items_dir().join(filename(row))
}

/// Write a file by writing a sibling temporary and renaming over the target.
///
/// A rename within a directory is atomic on every platform trck runs on, so an
/// interrupted run leaves the previous contents rather than a truncated file. The index
/// is the tracker's only source of truth; half of one is worse than none.
pub(crate) fn write_file(path: &Path, contents: &str) -> Result<(), String> {
    write_atomic(path, contents)
}

pub(crate) fn write_atomic(path: &Path, contents: &str) -> Result<(), String> {
    let tmp = path.with_extension(format!("tmp{}", std::process::id()));
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    std::fs::write(&tmp, contents).map_err(|e| format!("{}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(())
}

/// Apply a status transition and stamp the dates it implies.
///
/// Pure — no filesystem contact — so it is safe wherever the working tree may not be
/// settled: in-memory normalisation, dry runs, merge drivers.
pub(crate) fn apply_status(row: &mut Issue, new_status: &str) -> Result<(), String> {
    if let Some(msg) = config::check_status(new_status) {
        return Err(msg);
    }
    let was_initial = row.status == config::initial_status();
    row.status = new_status.to_string();
    if was_initial && new_status != config::initial_status() && row.started.is_none() {
        row.started = Some(now_utc()?);
    }
    if is_terminal(new_status) {
        if row.closed.is_none() {
            row.closed = Some(now_utc()?);
        }
    } else {
        // Reopening clears the whole closure record. Dropping the timestamp but keeping
        // the resolution would leave a row that is open and yet says *why* it closed —
        // a state `check` rejects, so the verb would be writing an invalid tracker.
        row.closed = None;
        row.resolution = None;
    }
    Ok(())
}

/// Rows ordered children-before-parents, so a bottom-up pass sees each node's
/// descendants already settled.
fn postorder(g: &Graph) -> Vec<String> {
    let mut out = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for r in &g.rows {
        // Explicit stack with a visit flag; recursion would blow up on the deep
        // hierarchy a malformed index can produce.
        let mut stack = vec![(r.id.clone(), false)];
        while let Some((id, expanded)) = stack.pop() {
            if expanded {
                out.push(id);
                continue;
            }
            if !seen.insert(id.clone()) {
                continue;
            }
            stack.push((id.clone(), true));
            for kid in g.children_of(&id) {
                stack.push((kid.clone(), false));
            }
        }
    }
    out
}

/// Persist, regenerate and derive. The single write path every mutating verb ends in.
///
/// The two normalisations happen here rather than in each verb, which is what makes the
/// rollup uniform: `points` is a leaf-only input, so a parent's is reset; and a parent's
/// status is derived from its children unless it is pinned with `manual_status`.
pub(crate) fn finalize(ctx: &Ctx, rows: Vec<Issue>) -> Result<Vec<Issue>, String> {
    let mut g = Graph::new(rows);

    let parent_ids: Vec<String> = g.rows.iter().map(|r| r.id.clone()).filter(|id| !g.is_leaf(id)).collect();
    for r in &mut g.rows {
        if parent_ids.contains(&r.id) {
            r.points = DEFAULT_POINTS;
        }
    }

    for id in postorder(&g) {
        let kids = g.children_of(&id).to_vec();
        if kids.is_empty() {
            continue;
        }
        let Some(row) = g.get(&id) else { continue };
        if row.manual_status {
            continue;
        }
        let statuses: Vec<String> = kids.iter().filter_map(|k| g.get(k).map(|r| r.status.clone())).collect();
        let desired = config::reconcile(&statuses);
        if g.get(&id).is_some_and(|r| r.status != desired) {
            let mut rows = std::mem::take(&mut g.rows);
            if let Some(row) = rows.iter_mut().find(|r| r.id == id) {
                apply_status(row, desired)?;
            }
            g = Graph::new(rows);
        }
    }

    write_atomic(&ctx.index_path(), &render_index(&g.rows))?;
    write_atomic(&ctx.summary_path(), &generate_summary(&g))?;

    // Validate what was just written, reusing the rows rather than re-parsing. A verb
    // that leaves the tracker inconsistent still succeeds — it did what it was asked —
    // but says so loudly, because the next thing that runs is usually a commit.
    if let Ok(report) = crate::validate::validate(ctx, &g.rows) {
        for w in &report.warnings {
            eprintln!("warning: {w}");
        }
        if !report.errors.is_empty() {
            eprintln!("\nINCONSISTENCIES after this operation:");
            for e in &report.errors {
                eprintln!("  error: {e}");
            }
            eprintln!("the tracker is now inconsistent — fix before committing.");
        }
    }
    Ok(g.rows)
}

pub(crate) fn load_rows(ctx: &Ctx) -> Result<Vec<Issue>, String> {
    let path = ctx.index_path();
    match std::fs::read_to_string(&path) {
        Ok(text) => parse_index(&text, "index.jsonl"),
        Err(_) => Ok(Vec::new()),
    }
}

/// Resolve a CLI id token to exactly one issue: exact id, then unique prefix.
pub(crate) fn resolve_ref(rows: &[Issue], token: &str) -> Result<String, String> {
    let token = token.strip_prefix('#').unwrap_or(token);
    if rows.iter().any(|r| r.id == token) {
        return Ok(token.to_string());
    }
    let hits: Vec<&str> = rows.iter().map(|r| r.id.as_str()).filter(|id| id.starts_with(token)).collect();
    match hits.len() {
        1 => Ok(hits[0].to_string()),
        0 => Err(format!("no issue matching '{token}'")),
        _ => {
            let mut cands = hits;
            cands.sort_unstable();
            Err(format!("ambiguous id prefix '{token}' matches: {}", cands.join(", ")))
        },
    }
}

// --------------------------------------------------------------------------- //
// the verbs
// --------------------------------------------------------------------------- //

#[cfg(test)]
mod tests {
    // Tests assert; that is their job. The crate denies unwrap/expect/panic because a
    // malformed tracker must produce a diagnostic rather than a stack trace, but a test
    // that cannot panic cannot fail.
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn reopening_clears_both_closed_and_resolution() {
        // Leaving a terminal status must clear the whole closure record, not just the
        // timestamp: a row that is 'ongoing' while carrying a resolution is one our own
        // `check` rejects, so keeping it would have the verb write an invalid tracker.
        let mut row = Issue {
            id: "aaaaaaa".into(),
            slug: "alpha".into(),
            title: "Alpha".into(),
            status: config::DONE.into(),
            priority: "medium".into(),
            points: 1,
            parent: None,
            labels: Vec::new(),
            depends_on: Vec::new(),
            spec: None,
            review_url: None,
            created: Some("2026-01-01T00:00:00Z".into()),
            started: Some("2026-01-01T00:00:00Z".into()),
            closed: Some("2026-01-01T00:00:00Z".into()),
            resolution: Some("wontfix".into()),
            manual_status: false,
            extra: BTreeMap::new(),
        };
        apply_status(&mut row, config::ONGOING).unwrap();
        assert_eq!(row.closed, None);
        assert_eq!(row.resolution, None, "resolution must not outlive the closure");
    }

    #[test]
    fn slugify_matches_the_python_rule() {
        assert_eq!(slugify("Fix the parser"), "fix-the-parser");
        assert_eq!(slugify("  Leading & trailing!  "), "leading-trailing");
        assert_eq!(slugify("CamelCase123"), "camelcase123");
        assert_eq!(slugify("---"), "");
        assert_eq!(slugify("café ✓"), "caf");
    }

    #[test]
    fn resolve_ref_takes_an_exact_id_then_a_unique_prefix() {
        let mk = |id: &str| {
            crate::issue::Issue::from_json(
                &crate::json::parse(&format!(r#"{{"id": "{id}", "slug": "s", "title": "T", "status": "backlog", "priority": "low"}}"#)).expect("json"),
            )
            .expect("issue")
        };
        let rows = vec![mk("aaaaaaa"), mk("aabbbbb")];
        assert_eq!(resolve_ref(&rows, "aaaaaaa").expect("exact"), "aaaaaaa");
        assert_eq!(resolve_ref(&rows, "#aaaaaaa").expect("hash"), "aaaaaaa");
        assert_eq!(resolve_ref(&rows, "aab").expect("prefix"), "aabbbbb");
        assert!(resolve_ref(&rows, "aa").expect_err("ambiguous").contains("ambiguous"));
        assert!(resolve_ref(&rows, "zz").expect_err("none").contains("no issue"));
    }
}

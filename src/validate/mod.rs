//! `check` — the contract enforcer the pre-commit hook runs.
//!
//! Everything here answers one question: is the index consistent with the files on disk,
//! with itself, and with the rules the verbs maintain? A violation means either a
//! hand-edit or a field-wise merge that resolved related fields independently — the
//! verbs cannot produce one.
//!
//! The split between error and warning is whether the tracker is *wrong* or merely
//! *odd*. A missing body file is wrong. A terminal issue depending on a non-terminal one
//! is odd — it happens when work is closed out of order, and saying so is more useful
//! than refusing to proceed.
//!
//! [`validate`] is the list of passes. [`row`] holds the per-row ones, [`checks`] the ones that
//! need the whole graph, and [`cycle`] the wording of the hardest of them to act on. What is
//! left here is finding the files to check against.

mod checks;
mod cycle;
mod row;

use checks::{check_cycles, check_references, check_rollups, warn_unfinished_dependencies};
pub(crate) use cycle::describe_cycle;
use row::check_row;

use crate::config;
use crate::discovery::Ctx;
use crate::graph::Graph;
use crate::issue::Issue;
use std::collections::{BTreeMap, BTreeSet};

/// id -> (slug, filename) for every issue markdown in the items dir.
type Files = BTreeMap<String, (String, String)>;

/// What `check` found. Errors fail the run; warnings are printed and tolerated.
pub(crate) struct Report {
    pub(crate) errors: Vec<String>,
    pub(crate) warnings: Vec<String>,
}

/// Every issue markdown in the items dir, keyed by id.
///
/// Status is not encoded in the path, so there is no folder component. Two files can
/// still claim one id through different slugs, which is fatal rather than a validation
/// error: it makes "the file for #x" ambiguous, and every later check would be guessing.
fn scan_files(ctx: &Ctx) -> Result<Files, String> {
    let mut found: Files = BTreeMap::new();
    let Ok(entries) = std::fs::read_dir(ctx.items_dir()) else {
        return Ok(found);
    };
    let mut names: Vec<String> = entries.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    names.sort();
    for name in names {
        let Some((id, slug)) = issue_filename(&name) else {
            continue;
        };
        if let Some((_, other)) = found.get(id) {
            return Err(format!("duplicate issue id {id} on disk: {other} and {name}"));
        }
        found.insert(id.to_string(), (slug.to_string(), name.clone()));
    }
    Ok(found)
}

/// `<id>-<slug>.md` split into its two halves, or `None` when the name is not one.
///
/// Only well-formed issue filenames count: a README or a scratch note parked in `items/` must
/// not be mistaken for an issue and then reported as one missing its index row. The id is
/// lowercase alphanumeric, the slug is slug-shaped.
fn issue_filename(name: &str) -> Option<(&str, &str)> {
    let stem = name.strip_suffix(".md")?;
    let (id, slug) = stem.split_once('-')?;
    let id_ok = !id.is_empty() && id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit());
    (id_ok && is_slug(slug)).then_some((id, slug))
}

fn is_slug(s: &str) -> bool {
    let mut chars = s.chars();
    chars.next().is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit()) && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// Validate the index against the on-disk files and against itself.
pub(crate) fn validate(ctx: &Ctx, rows: &[Issue]) -> Result<Report, String> {
    let mut errors: Vec<String> = Vec::new();
    let mut warnings: Vec<String> = config::vestigial_warnings(&ctx.config);
    let files = scan_files(ctx)?;
    let g = Graph::new(rows.to_vec());
    let by_id: BTreeSet<&str> = g.rows.iter().map(|r| r.id.as_str()).collect();

    for r in &g.rows {
        check_row(&g, r, &files, &mut errors);
    }
    check_references(&g, &by_id, &files, &mut errors);
    check_cycles(&g, &mut errors);
    check_rollups(&g, &mut errors);
    warn_unfinished_dependencies(&g, &mut warnings);
    Ok(Report { errors, warnings })
}

#[cfg(test)]
mod tests {
    // Tests assert; that is their job. The crate denies unwrap/expect/panic because a
    // malformed tracker must produce a diagnostic rather than a stack trace, but a test
    // that cannot panic cannot fail.
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    #[test]
    fn a_slug_must_be_filename_safe() {
        assert!(is_slug("fix-the-parser"));
        assert!(is_slug("a1"));
        assert!(!is_slug("-leading"));
        assert!(!is_slug("Upper"));
        assert!(!is_slug("has space"));
        assert!(!is_slug(""));
    }

    /// The gate that keeps a stray file in `items/` from being read as an issue — and then
    /// reported as one whose index row is missing.
    #[test]
    fn only_a_well_formed_issue_filename_is_an_issue() {
        assert_eq!(issue_filename("k3m9x2a-fix-the-parser.md"), Some(("k3m9x2a", "fix-the-parser")));
        assert_eq!(issue_filename("a1-b.md"), Some(("a1", "b")));
        for not_one in [
            "README.md",         // no id-slug split
            "notes.txt",         // not markdown
            "-leading.md",       // empty id
            "UPPER-slug.md",     // id is not lowercase alphanumeric
            "a_b-slug.md",       // underscore is not an id character
            "k3m9x2a-Upper.md",  // slug is not slug-shaped
            "k3m9x2a-has it.md", // nor is that
            "k3m9x2a.md",        // no slug at all
        ] {
            assert_eq!(issue_filename(not_one), None, "{not_one} should not read as an issue");
        }
    }

    /// A README *does* contain a dash in some repos, so the slug shape is what excludes it —
    /// not the absence of one.
    #[test]
    fn a_dashed_non_issue_file_is_still_excluded() {
        assert_eq!(issue_filename("release-notes.md"), Some(("release", "notes")), "this one is indistinguishable by shape");
        assert_eq!(issue_filename("Release-Notes.md"), None);
    }
}

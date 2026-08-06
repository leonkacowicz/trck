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

use crate::config::{self, is_terminal};
use crate::discovery::Ctx;
use crate::graph::Graph;
use crate::issue::{DEFAULT_POINTS, Issue};
use crate::json::Json;
use crate::summary::filename;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

/// What `check` found. Errors fail the run; warnings are printed and tolerated.
pub(crate) struct Report {
    pub(crate) errors: Vec<String>,
    pub(crate) warnings: Vec<String>,
}

/// Map id -> (slug, filename) for every issue markdown in the items dir.
///
/// Status is not encoded in the path, so there is no folder component. Two files can
/// still claim one id through different slugs, which is fatal rather than a validation
/// error: it makes "the file for #x" ambiguous, and every later check would be guessing.
fn scan_files(ctx: &Ctx) -> Result<BTreeMap<String, (String, String)>, String> {
    let mut found: BTreeMap<String, (String, String)> = BTreeMap::new();
    let Ok(entries) = std::fs::read_dir(ctx.items_dir()) else {
        return Ok(found);
    };
    let mut names: Vec<String> = entries.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    names.sort();
    for name in names {
        let Some(stem) = name.strip_suffix(".md") else {
            continue;
        };
        let Some((id, slug)) = stem.split_once('-') else {
            continue;
        };
        // Only well-formed issue filenames count. A README or a scratch note parked in
        // `items/` must not be mistaken for an issue and reported as one missing its
        // index row — the id is lowercase alphanumeric, the slug is slug-shaped.
        if id.is_empty() || !id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()) {
            continue;
        }
        if !is_slug(slug) {
            continue;
        }
        if let Some((_, other)) = found.get(id) {
            return Err(format!("duplicate issue id {id} on disk: {other} and {name}"));
        }
        found.insert(id.to_string(), (slug.to_string(), name.clone()));
    }
    Ok(found)
}

/// A human-readable reason for an effective cycle: the node loop plus the authored edges
/// and parent links that induce it. The loop itself is implied and was never typed, so
/// naming only it would leave nothing to go and fix.
#[allow(clippy::many_single_char_names, reason = "u/v are the loop edge, a/b the witness")]
pub(crate) fn describe_cycle(g: &Graph, cyc: &[String]) -> String {
    let mut seq: Vec<&String> = cyc.iter().collect();
    if let Some(first) = cyc.first() {
        seq.push(first);
    }
    let chain = seq.iter().map(|c| format!("#{c}")).collect::<Vec<_>>().join(" -> ");
    let mut authored: Vec<String> = Vec::new();
    let mut notes: Vec<String> = Vec::new();
    for pair in seq.windows(2) {
        let (u, v) = (pair[0], pair[1]);
        // The witness: the authored edge (a -> b) with a an ancestor-or-self of u and v
        // inside subtree(b), which is what makes u reach v.
        let mut witness = None;
        'outer: for a in std::iter::once(u.clone()).chain(g.ancestors_of(u)) {
            for b in g.requires_of(&a) {
                if g.subtree(&b).contains(v) {
                    witness = Some((a.clone(), b.clone()));
                    break 'outer;
                }
            }
        }
        let Some((a, b)) = witness else { continue };
        let edge = format!("#{a} -> #{b}");
        if !authored.contains(&edge) {
            authored.push(edge);
        }
        if a != *u {
            notes.push(format!("#{u} inherits #{a}'s deps"));
        }
        if b != *v {
            notes.push(format!("#{v} is under #{b}"));
        }
    }
    let mut reason = chain;
    if !authored.is_empty() {
        let _ = write!(reason, "; authored: {}", authored.join(", "));
    }
    if !notes.is_empty() {
        let mut seen = BTreeSet::new();
        let unique: Vec<&String> = notes.iter().filter(|n| seen.insert((*n).clone())).collect();
        let _ = write!(reason, "; {}", unique.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", "));
    }
    reason
}

fn is_slug(s: &str) -> bool {
    let mut chars = s.chars();
    chars.next().is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit()) && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

fn is_field_key(k: &str) -> bool {
    let mut chars = k.chars();
    chars.next().is_some_and(|c| c.is_ascii_lowercase()) && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
}

/// Python's `repr` of a custom-field value, so the two engines word the same complaint
/// the same way.
fn repr(v: &Json) -> String {
    match v {
        Json::Null => "None".into(),
        Json::Bool(true) => "True".into(),
        Json::Bool(false) => "False".into(),
        Json::Number(raw) => raw.clone(),
        Json::String(s) => format!("'{s}'"),
        Json::Array(items) => format!("[{}]", items.iter().map(repr).collect::<Vec<_>>().join(", ")),
        Json::Object(pairs) => format!("{{{}}}", pairs.iter().map(|(k, v)| format!("'{k}': {}", repr(v))).collect::<Vec<_>>().join(", ")),
    }
}

/// Per-row checks: the file it should have, the values it may carry, and the invariants
/// no verb would break.
fn check_row(g: &Graph, r: &Issue, files: &BTreeMap<String, (String, String)>, errors: &mut Vec<String>) {
    let iid = &r.id;
    let Some((slug, fname)) = files.get(iid) else {
        errors.push(format!("#{iid} in index but no markdown file on disk"));
        return;
    };
    if r.slug != *slug {
        errors.push(format!("#{iid} index slug '{}' != filename slug '{slug}'", r.slug));
    }
    if *fname != filename(r) {
        errors.push(format!("#{iid} filename '{fname}' != expected '{}'", filename(r)));
    }
    if r.slug.is_empty() || !is_slug(&r.slug) {
        errors.push(format!("#{iid} bad slug '{}'", r.slug));
    }
    if !config::STATUSES.contains(&r.status.as_str()) {
        errors.push(format!("#{iid} unknown status '{}'", r.status));
    }
    if let Some(m) = config::check_priority(&r.priority) {
        errors.push(format!("#{iid} {m}"));
    }
    if g.is_leaf(iid) {
        if let Some(m) = config::check_points(r.points) {
            errors.push(format!("#{iid} {m}"));
        }
    } else if r.points != DEFAULT_POINTS {
        errors.push(format!("#{iid} has children but carries points {} (derived from leaves, must be unset)", r.points));
    }
    if let Some(res) = &r.resolution
        && let Some(m) = config::check_resolution(res)
    {
        errors.push(format!("#{iid} {m}"));
    }
    // `(status, closed, resolution)` is one unit: a move to a non-terminal status clears
    // both dates, and `--resolution` is refused unless the target is terminal. So a
    // non-terminal row carrying either is a row no verb can have written — a hand-edit,
    // or a field-wise merge that resolved the tuple's members independently. Two
    // separate errors, because a merge can produce either alone.
    //
    // `review_url` is deliberately not in this set: a closed issue keeping its link is
    // the review record for the change that resolved it.
    if !is_terminal(&r.status) {
        if let Some(res) = &r.resolution {
            errors.push(format!("#{iid} is '{}' (not terminal) but carries resolution '{res}'", r.status));
        }
        if let Some(closed) = &r.closed {
            errors.push(format!("#{iid} is '{}' (not terminal) but carries closed '{closed}'", r.status));
        }
    }
    if let Some(url) = &r.review_url
        && let Some(m) = config::check_review_url(url)
    {
        errors.push(format!("#{iid} {m}"));
    }
    for (k, v) in &r.extra {
        if is_field_key(k) {
            if !matches!(v, Json::String(_)) {
                errors.push(format!("#{iid} custom field '{k}' must be a string, got {}", repr(v)));
            }
        } else {
            errors.push(format!("#{iid} bad custom field key '{k}'"));
        }
    }
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
    for id in files.keys() {
        if !by_id.contains(id.as_str()) {
            errors.push(format!("#{id} markdown file on disk but no index row"));
        }
    }
    for r in &g.rows {
        if let Some(p) = &r.parent
            && !by_id.contains(p.as_str())
        {
            errors.push(format!("#{} parent #{p} does not exist", r.id));
        }
        for dep in &r.depends_on {
            if !by_id.contains(dep.as_str()) {
                errors.push(format!("#{} depends_on #{dep} which does not exist", r.id));
            }
        }
    }

    // One error per cycle, not one per node.
    for cyc in g.parent_cycles() {
        let mut chain: Vec<String> = cyc.iter().map(|c| format!("#{c}")).collect();
        if let Some(first) = cyc.first() {
            chain.push(format!("#{first}"));
        }
        errors.push(format!("parent cycle: {}", chain.join(" -> ")));
    }
    // Effective cycles are a superset of the authored ones, and surface inherited
    // deadlocks that arrived by hand-edit, import or `mv`.
    for cyc in g.effective_cycles() {
        errors.push(format!("effective dependency cycle: {}", describe_cycle(&g, &cyc)));
    }

    // A non-pinned parent's status must equal the rollup of its children. `finalize`
    // maintains this after every verb, so a violation means a hand-edited index.
    for r in &g.rows {
        let kids = g.children_of(&r.id);
        if kids.is_empty() || r.manual_status {
            continue;
        }
        let statuses: Vec<String> = kids.iter().filter_map(|k| g.get(k).map(|c| c.status.clone())).collect();
        let desired = config::reconcile(&statuses);
        if r.status != desired {
            errors.push(format!(
                "#{} status '{}' should be '{desired}' (derived from its children; \
                 pin it with a manual `mv` to override)",
                r.id, r.status
            ));
        }
    }
    for r in &g.rows {
        if is_terminal(&r.status) {
            for dep in &r.depends_on {
                if g.get(dep).is_some_and(|d| !is_terminal(&d.status)) {
                    warnings.push(format!("#{} is terminal but depends on non-terminal #{dep}", r.id));
                }
            }
        }
    }
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

    #[test]
    fn a_field_key_must_be_slug_like_but_may_hold_underscores() {
        assert!(is_field_key("assignee"));
        assert!(is_field_key("due_date"));
        assert!(!is_field_key("1st"));
        assert!(!is_field_key("Assignee"));
    }

    #[test]
    fn repr_matches_pythons_wording() {
        // A fixture asserting stderr should not care which engine produced it.
        assert_eq!(repr(&Json::Bool(true)), "True");
        assert_eq!(repr(&Json::Null), "None");
        assert_eq!(repr(&Json::Number("3".into())), "3");
        assert_eq!(repr(&Json::String("x".into())), "'x'");
        assert_eq!(repr(&Json::Array(vec![Json::Number("1".into()), Json::Null])), "[1, None]");
    }
}

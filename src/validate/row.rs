//! The per-row checks: the file a row should have, the values it may carry, and the invariants
//! no verb would break.
//!
//! **The order of the checks is the order of the output.** `check` prints errors as they are
//! pushed, and the conformance suite compares that literally — so [`check_row`] is a sequence
//! of named checks rather than a set, and adding one means deciding where it goes.

use super::{Files, is_slug};
use crate::config::{self, is_terminal};
use crate::graph::Graph;
use crate::issue::{DEFAULT_POINTS, Issue};
use crate::json::Json;
use crate::summary::filename;

pub(super) fn check_row(g: &Graph, r: &Issue, files: &Files, errors: &mut Vec<String>) {
    let Some((slug, fname)) = files.get(&r.id) else {
        errors.push(format!("#{} in index but no markdown file on disk", r.id));
        return; // every check below reads the file's name; there is nothing to compare against
    };
    check_naming(r, slug, fname, errors);
    check_vocabulary(r, errors);
    check_points(g, r, errors);
    check_closure(r, errors);
    check_review_url(r, errors);
    check_custom_fields(r, errors);
}

/// The slug must agree three ways: with the filename's slug, with the name the verbs would
/// write, and with the shape a filename can hold at all.
fn check_naming(r: &Issue, slug: &str, fname: &str, errors: &mut Vec<String>) {
    let iid = &r.id;
    if r.slug != slug {
        errors.push(format!("#{iid} index slug '{}' != filename slug '{slug}'", r.slug));
    }
    if fname != filename(r) {
        errors.push(format!("#{iid} filename '{fname}' != expected '{}'", filename(r)));
    }
    if r.slug.is_empty() || !is_slug(&r.slug) {
        errors.push(format!("#{iid} bad slug '{}'", r.slug));
    }
}

/// The two fixed vocabularies. Neither is configurable, so a value outside one can only be a
/// hand-edit or a row written by an engine that had different words.
fn check_vocabulary(r: &Issue, errors: &mut Vec<String>) {
    let iid = &r.id;
    if !config::STATUSES.contains(&r.status.as_str()) {
        errors.push(format!("#{iid} unknown status '{}'", r.status));
    }
    if let Some(m) = config::check_priority(&r.priority) {
        errors.push(format!("#{iid} {m}"));
    }
}

/// A leaf's points are its own; a parent's are the sum of its leaves, so points stored on a
/// parent would be double-counted and are refused rather than ignored.
fn check_points(g: &Graph, r: &Issue, errors: &mut Vec<String>) {
    let iid = &r.id;
    if g.is_leaf(iid) {
        if let Some(m) = config::check_points(r.points) {
            errors.push(format!("#{iid} {m}"));
        }
    } else if r.points != DEFAULT_POINTS {
        errors.push(format!("#{iid} has children but carries points {} (derived from leaves, must be unset)", r.points));
    }
}

/// `(status, closed, resolution)` is one unit.
///
/// A move to a non-terminal status clears both dates, and `--resolution` is refused unless the
/// target is terminal. So a non-terminal row carrying either is a row no verb can have written
/// — a hand-edit, or a field-wise merge that resolved the tuple's members independently. Two
/// separate errors, because a merge can produce either alone.
///
/// `review_url` is deliberately not in this set: a closed issue keeping its link is the review
/// record for the change that resolved it.
fn check_closure(r: &Issue, errors: &mut Vec<String>) {
    let iid = &r.id;
    if let Some(res) = &r.resolution
        && let Some(m) = config::check_resolution(res)
    {
        errors.push(format!("#{iid} {m}"));
    }
    if is_terminal(&r.status) {
        return;
    }
    if let Some(res) = &r.resolution {
        errors.push(format!("#{iid} is '{}' (not terminal) but carries resolution '{res}'", r.status));
    }
    if let Some(closed) = &r.closed {
        errors.push(format!("#{iid} is '{}' (not terminal) but carries closed '{closed}'", r.status));
    }
}

fn check_review_url(r: &Issue, errors: &mut Vec<String>) {
    if let Some(url) = &r.review_url
        && let Some(m) = config::check_review_url(url)
    {
        errors.push(format!("#{} {m}", r.id));
    }
}

/// A custom field must be a string under a slug-like key. Anything else round-trips fine but
/// cannot be filtered, sorted or shown, so it would be a field that exists and does nothing.
fn check_custom_fields(r: &Issue, errors: &mut Vec<String>) {
    let iid = &r.id;
    for (k, v) in &r.extra {
        if !is_field_key(k) {
            errors.push(format!("#{iid} bad custom field key '{k}'"));
        } else if !matches!(v, Json::String(_)) {
            errors.push(format!("#{iid} custom field '{k}' must be a string, got {}", repr(v)));
        }
    }
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

#[cfg(test)]
mod tests {
    // Tests assert; that is their job. The crate denies unwrap/expect/panic because a
    // malformed tracker must produce a diagnostic rather than a stack trace, but a test
    // that cannot panic cannot fail.
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;
    use crate::test_graph::graph;
    use std::collections::BTreeMap;

    /// Run the row checks with the on-disk file agreeing with the row, so only the checks
    /// under test can complain.
    fn errors_for(spec: &str) -> Vec<String> {
        let g = graph(&[spec]);
        let r = g.rows.first().expect("one row").clone();
        let mut files: Files = BTreeMap::new();
        files.insert(r.id.clone(), (r.slug.clone(), filename(&r)));
        let mut errors = Vec::new();
        check_row(&g, &r, &files, &mut errors);
        errors
    }

    #[test]
    fn a_well_formed_row_says_nothing() {
        assert!(errors_for("aaaaaaa").is_empty());
    }

    /// A row with no file cannot be checked further: everything after that reads the filename.
    #[test]
    fn a_missing_file_stops_the_rest() {
        let g = graph(&["aaaaaaa @nonsense"]);
        let r = g.rows.first().expect("one row").clone();
        let mut errors = Vec::new();
        check_row(&g, &r, &BTreeMap::new(), &mut errors);
        assert_eq!(errors, ["#aaaaaaa in index but no markdown file on disk"], "the bad status must not also be reported");
    }

    #[test]
    fn an_unknown_status_or_priority_is_named() {
        assert!(errors_for("aaaaaaa @nonsense").iter().any(|e| e.contains("unknown status 'nonsense'")));
        assert!(errors_for("aaaaaaa !nope").iter().any(|e| e.contains("bad priority 'nope'")));
    }

    /// A parent's points are derived, so storing any is refused — but the default is what an
    /// untouched row carries and must not be.
    #[test]
    fn a_parent_may_not_carry_points() {
        let g = graph(&["aaaaaaa #7", "bbbbbbb:aaaaaaa"]);
        let parent = g.rows.first().expect("parent").clone();
        let mut files: Files = BTreeMap::new();
        for r in &g.rows {
            files.insert(r.id.clone(), (r.slug.clone(), filename(r)));
        }
        let mut errors = Vec::new();
        check_row(&g, &parent, &files, &mut errors);
        assert!(errors.iter().any(|e| e.contains("has children but carries points 7")), "{errors:?}");
    }

    /// The tuple splits into two errors on purpose: a field-wise merge can produce either
    /// half alone, and saying so names the one that needs fixing.
    #[test]
    fn a_non_terminal_row_carrying_closure_fields_reports_each_separately() {
        let mut r = graph(&["aaaaaaa"]).rows.first().expect("row").clone();
        r.resolution = Some("wontfix".into());
        r.closed = Some("2026-01-01T00:00:00Z".into());
        let mut errors = Vec::new();
        check_closure(&r, &mut errors);
        assert_eq!(errors.len(), 2, "{errors:?}");
        assert!(errors[0].contains("carries resolution"), "{errors:?}");
        assert!(errors[1].contains("carries closed"), "{errors:?}");
    }

    /// A terminal row keeping both is exactly what `done` writes, so it must stay silent.
    #[test]
    fn a_terminal_row_may_carry_both() {
        let mut r = graph(&["aaaaaaa @done"]).rows.first().expect("row").clone();
        r.resolution = Some("wontfix".into());
        r.closed = Some("2026-01-01T00:00:00Z".into());
        let mut errors = Vec::new();
        check_closure(&r, &mut errors);
        assert!(errors.is_empty(), "{errors:?}");
    }

    #[test]
    fn a_custom_field_must_be_a_string_under_a_slug_like_key() {
        let mut r = graph(&["aaaaaaa"]).rows.first().expect("row").clone();
        r.extra.insert("Bad".into(), Json::String("v".into()));
        r.extra.insert("n".into(), Json::Number("2".into()));
        r.extra.insert("ok".into(), Json::String("v".into()));
        let mut errors = Vec::new();
        check_custom_fields(&r, &mut errors);
        assert_eq!(errors.len(), 2, "{errors:?}");
        assert!(errors.iter().any(|e| e.contains("bad custom field key 'Bad'")), "{errors:?}");
        assert!(errors.iter().any(|e| e.contains("custom field 'n' must be a string, got 2")), "{errors:?}");
    }

    /// A bad key is reported *instead of* the type complaint, not as well as: one problem per
    /// field, and the key is the one to fix first.
    #[test]
    fn a_bad_key_is_reported_once_even_when_the_value_is_also_wrong() {
        let mut r = graph(&["aaaaaaa"]).rows.first().expect("row").clone();
        r.extra.insert("Bad".into(), Json::Number("1".into()));
        let mut errors = Vec::new();
        check_custom_fields(&r, &mut errors);
        assert_eq!(errors, ["#aaaaaaa bad custom field key 'Bad'"]);
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

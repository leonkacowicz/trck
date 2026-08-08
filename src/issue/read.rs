//! Parsing one index row, and rewriting the shapes an older engine wrote.

use super::coerce::{list_of, opt_str, want_id, want_label, want_str};
use super::diagnostic::{bad, py_repr};
use super::row::Row;
use super::{DEFAULT_POINTS, Issue};
use crate::json::Json;
use std::collections::BTreeMap;

impl Issue {
    /// Parse one index row.
    ///
    /// Loud and specific on failure: a row missing a required field or carrying a
    /// wrongly typed value is not a well-formed issue, and guessing at it is how a
    /// tracker silently loses data.
    pub(crate) fn from_json(raw: &Json) -> Result<Issue, String> {
        let Json::Object(pairs) = raw else {
            return Err(not_an_object(raw));
        };
        let mut row = Row::new(pairs);
        let migrated_labels = migrate(&mut row);
        require_present(&row)?;

        // The three steps run in this order deliberately, and so does each one internally. A
        // row can be wrong in several places at once and only the first complaint is
        // reported, so the read order *is* the diagnostic — and the conformance goldens name
        // which field a doubly-broken row blames.
        let mut issue = read_required(&row)?;
        read_defaulted(&row, &mut issue)?;
        read_optional_strings(&row, &mut issue)?;

        for name in migrated_labels {
            if !issue.labels.contains(&name) {
                issue.labels.push(name);
            }
        }
        issue.extra = row.into_extra();
        Ok(issue)
    }
}

/// The five required fields, with everything else left at its default — which is exactly what
/// a row mentioning nothing else means. The defaults are spelled out here, once.
fn read_required(row: &Row) -> Result<Issue, String> {
    Ok(Issue {
        id: want_id("id", &row.required("id"))?,
        slug: want_str("slug", &row.required("slug"))?,
        title: want_str("title", &row.required("title"))?,
        status: crate::config::canonical_status(&want_str("status", &row.required("status"))?).to_string(),
        priority: want_str("priority", &row.required("priority"))?,
        points: DEFAULT_POINTS,
        parent: None,
        labels: Vec::new(),
        depends_on: Vec::new(),
        spec: None,
        review_url: None,
        created: None,
        started: None,
        closed: None,
        resolution: None,
        manual_status: false,
        extra: BTreeMap::new(),
    })
}

/// The fields that carry a non-`None` default, so absence is meaningful rather than empty.
fn read_defaulted(row: &Row, issue: &mut Issue) -> Result<(), String> {
    issue.points = read_points(row)?;
    issue.parent = row.present("parent").map(|v| want_id("parent", v)).transpose()?;
    issue.labels = list_of(row, "labels", want_label)?;
    issue.depends_on = list_of(row, "depends_on", want_id)?;
    issue.manual_status = read_manual_status(row)?;
    Ok(())
}

/// The optional strings, all read the same way, in canonical order.
fn read_optional_strings(row: &Row, issue: &mut Issue) -> Result<(), String> {
    issue.spec = opt_str(row, "spec")?;
    issue.review_url = opt_str(row, "review_url")?;
    issue.created = opt_str(row, "created")?;
    issue.started = opt_str(row, "started")?;
    issue.closed = opt_str(row, "closed")?;
    issue.resolution = opt_str(row, "resolution")?;
    Ok(())
}

/// A row that is not an object at all. Python calls a JSON array a "list", and these two
/// engines' messages have to agree, so the name is spelled out rather than taken from
/// [`Json::type_name`].
fn not_an_object(raw: &Json) -> String {
    let what = match raw {
        Json::Array(_) => "list",
        other => other.type_name(),
    };
    format!("expected a JSON object, got {what}")
}

/// The five fields a well-formed row cannot omit, checked before anything is coerced so a
/// missing field is never reported as a type error.
fn require_present(row: &Row) -> Result<(), String> {
    for key in ["id", "slug", "title", "status", "priority"] {
        if row.present(key).is_none() {
            return Err(bad(key, "is required"));
        }
    }
    Ok(())
}

/// Absent means the default weight, not zero — an unpointed tracker's rollup is then a plain
/// issue count. An explicit `0` is meaningful and kept.
fn read_points(row: &Row) -> Result<i64, String> {
    match row.get("points") {
        None => Ok(DEFAULT_POINTS),
        Some(v) => v.as_i64().ok_or_else(|| bad("points", &format!("must be an integer, got {}", py_repr(v)))),
    }
}

/// Absent is false; anything that is not a boolean is refused rather than tested for
/// truthiness, so the field cannot mean one thing here and another somewhere else.
fn read_manual_status(row: &Row) -> Result<bool, String> {
    match row.get("manual_status") {
        None => Ok(false),
        Some(Json::Bool(b)) => Ok(*b),
        Some(other) => Err(bad("manual_status", &format!("must be a boolean, got {}", py_repr(other)))),
    }
}

/// Rewrite the shapes an older engine wrote. Returns labels the migration adds.
///
/// Read-time only, so an unmigrated tracker keeps working and is rewritten on its next
/// mutation — no flag day, and no migration verb to remember to run.
fn migrate(row: &mut Row) -> Vec<String> {
    let mut labels = Vec::new();
    // `milestone` was a single-valued field where a label does the job better.
    if let Some(Json::String(name)) = row.take("milestone")
        && !name.is_empty()
    {
        labels.push(name);
    }
    // `pr` -> `review_url`. The field was named for the common case, but what it
    // records is wherever the in-review output is being judged.
    if let Some(legacy) = row.take("pr")
        && !matches!(legacy, Json::Null)
        && row.present("review_url").is_none()
    {
        row.set("review_url", legacy);
    }
    labels
}

#[cfg(test)]
mod tests {
    // Tests assert; that is their job. The crate denies unwrap/expect/panic because a
    // malformed tracker must produce a diagnostic rather than a stack trace, but a test
    // that cannot panic cannot fail.
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;
    use crate::json::parse;

    fn issue(text: &str) -> Result<Issue, String> {
        Issue::from_json(&parse(text).expect("valid JSON"))
    }

    const MINIMAL: &str = r#"{"id": "k3m9x2a", "slug": "s", "title": "T",
                              "status": "backlog", "priority": "high"}"#;

    #[test]
    fn reads_the_required_fields() {
        let r = issue(MINIMAL).expect("parses");
        assert_eq!(r.id, "k3m9x2a");
        assert_eq!(r.points, DEFAULT_POINTS);
        assert!(r.labels.is_empty());
        assert!(!r.manual_status);
    }

    #[test]
    fn a_missing_required_field_is_an_error() {
        for key in ["id", "slug", "title", "status", "priority"] {
            let raw = MINIMAL.replace(&format!("\"{key}\""), "\"other\"");
            let err = issue(&raw).expect_err("should reject");
            assert!(err.contains(key), "{key}: {err}");
            assert!(err.contains("is required"), "{key}: {err}");
        }
    }

    /// An explicit `null` in a required field is *missing*, not wrongly typed — the whole
    /// reason `present` exists.
    #[test]
    fn an_explicit_null_required_field_reads_as_missing() {
        let raw = MINIMAL.replace(r#""k3m9x2a""#, "null");
        assert_eq!(issue(&raw).expect_err("rejected"), "field 'id' is required");
    }

    #[test]
    fn an_integer_id_is_rejected() {
        // Integer ids were the first iteration and are gone. The format carries strings.
        let err = issue(
            r#"{"id": 24, "slug": "s", "title": "T",
                            "status": "backlog", "priority": "high"}"#,
        )
        .expect_err("should reject");
        assert_eq!(err, "field 'id' must be a string id, got 24");
    }

    #[test]
    fn wrongly_typed_fields_name_the_field_and_the_value() {
        let cases = [
            (r#""points": "lots""#, "field 'points' must be an integer, got 'lots'"),
            (r#""labels": "x""#, "field 'labels' must be a list, got 'x'"),
            (r#""labels": [1]"#, "field 'labels' must contain only strings, got 1"),
            (r#""manual_status": "yes""#, "field 'manual_status' must be a boolean, got 'yes'"),
            (r#""slug": 3"#, "field 'slug' must be a string, got 3"),
        ];
        for (fragment, want) in cases {
            let raw = format!(
                r#"{{"id": "k3m9x2a", "slug": "s", "title": "T", "status": "backlog",
                     "priority": "high", {fragment}}}"#
            );
            let raw = raw.replace(r#""slug": "s", "#, if fragment.starts_with(r#""slug""#) { "" } else { r#""slug": "s", "# });
            assert_eq!(issue(&raw).expect_err("should reject"), want);
        }
    }

    /// Which field a doubly-broken row blames is the read order, and it is asserted rather
    /// than left to whichever line happens to come first after an edit.
    #[test]
    fn the_first_complaint_follows_the_read_order() {
        let raw = MINIMAL.replace(r#""priority": "high""#, r#""priority": "high", "spec": 1, "manual_status": 2"#);
        assert_eq!(issue(&raw).expect_err("rejected"), "field 'manual_status' must be a boolean, got 2");
        // And a missing field outranks any type error, because the check runs first.
        let raw = r#"{"slug": "s", "title": "T", "status": "backlog", "priority": "high", "points": "x"}"#;
        assert_eq!(issue(raw).expect_err("rejected"), "field 'id' is required");
    }

    #[test]
    fn an_empty_id_is_rejected() {
        let raw = MINIMAL.replace("k3m9x2a", "");
        assert_eq!(issue(&raw).expect_err("should reject"), "field 'id' must not be empty");
    }

    #[test]
    fn a_non_object_row_is_an_error() {
        assert_eq!(Issue::from_json(&parse("[1]").expect("valid")).expect_err("rejected"), "expected a JSON object, got list");
    }

    /// Python names a JSON array a "list"; every other shape uses its JSON name. Both engines
    /// have to say the same thing.
    #[test]
    fn a_non_object_row_names_the_shape_pythons_way() {
        for (doc, want) in [("[1]", "list"), ("\"s\"", "string"), ("42", "number"), ("null", "null"), ("true", "bool")] {
            let err = Issue::from_json(&parse(doc).expect("valid")).expect_err("rejected");
            assert_eq!(err, format!("expected a JSON object, got {want}"), "{doc}");
        }
    }

    #[test]
    fn milestone_migrates_to_a_label() {
        let raw = MINIMAL.replace(r#""priority": "high""#, r#""priority": "high", "milestone": "v1.0""#);
        let r = issue(&raw).expect("parses");
        assert_eq!(r.labels, ["v1.0"]);
        assert!(!r.extra.contains_key("milestone"));
    }

    /// An empty `milestone` adds nothing, and one the row already carries is not duplicated.
    #[test]
    fn a_migrated_label_is_neither_empty_nor_doubled() {
        let empty = MINIMAL.replace(r#""priority": "high""#, r#""priority": "high", "milestone": """#);
        assert!(issue(&empty).expect("parses").labels.is_empty());
        let dup = MINIMAL.replace(r#""priority": "high""#, r#""priority": "high", "milestone": "v1", "labels": ["v1"]"#);
        assert_eq!(issue(&dup).expect("parses").labels, ["v1"]);
    }

    #[test]
    fn pr_migrates_to_review_url() {
        let raw = MINIMAL.replace(r#""priority": "high""#, r#""priority": "high", "pr": "https://example.test/pull/1""#);
        let r = issue(&raw).expect("parses");
        assert_eq!(r.review_url.as_deref(), Some("https://example.test/pull/1"));
        assert!(!r.extra.contains_key("pr"));
        assert!(!r.to_canonical().to_json().contains("\"pr\""));
    }

    #[test]
    fn an_explicit_review_url_beats_a_stale_pr() {
        let raw = MINIMAL.replace(r#""priority": "high""#, r#""priority": "high", "pr": "https://old", "review_url": "https://new""#);
        assert_eq!(issue(&raw).expect("parses").review_url.as_deref(), Some("https://new"));
    }

    /// A `null` `pr` is dropped rather than migrated onto `review_url` as a null.
    #[test]
    fn a_null_pr_migrates_to_nothing() {
        let raw = MINIMAL.replace(r#""priority": "high""#, r#""priority": "high", "pr": null"#);
        let r = issue(&raw).expect("parses");
        assert_eq!(r.review_url, None);
        assert!(!r.extra.contains_key("pr"), "and it does not survive as an unknown key");
    }

    #[test]
    fn ongoing_migrates_to_in_progress() {
        // A tracker written by an older engine: read under the current name, and written
        // back under it, so the next index write converts the row for good.
        let raw = MINIMAL.replace(r#""status": "backlog""#, r#""status": "ongoing""#);
        let r = issue(&raw).expect("parses");
        assert_eq!(r.status, "in-progress");
        assert!(!r.to_canonical().to_json().contains("ongoing"));
    }

    #[test]
    fn a_non_default_value_is_kept() {
        // The test is equals-the-default, not falsiness: points 0 is meaningful.
        let raw = MINIMAL.replace(r#""priority": "high""#, r#""priority": "high", "points": 0"#);
        assert!(issue(&raw).expect("parses").to_canonical().to_json().contains(r#""points": 0"#));
    }
}

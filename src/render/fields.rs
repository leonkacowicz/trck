//! Reading one field off an issue as displayable text, built-in or custom.
//!
//! The awkward part is that "absent" means two different things depending on who is asking, so
//! there are two entry points over one set of lookups. See [`field_value_raw`].

use super::python_list;
use crate::issue::{CANON_KEYS, Issue};
use crate::json::Json;

/// One field value, built-in or custom, or `None` when the field is genuinely absent.
///
/// An **empty string is a value**, not an absence: `--field note=` sets one and the index
/// keeps it, so `show` must display it and a `--field note=` filter must match it. That is
/// the difference from [`field_value`], which additionally drops empties because a column
/// showing `note=` for every row carries no information.
pub(crate) fn field_value_raw(r: &Issue, name: &str) -> Option<String> {
    if CANON_KEYS.contains(&name) { field_value(r, name) } else { extra_value(r, name, true) }
}

/// One displayable field value, built-in or custom, or `None` when unset or empty.
pub(crate) fn field_value(r: &Issue, name: &str) -> Option<String> {
    if let Some(text) = required_text(r, name) {
        return non_empty(text);
    }
    if let Some(opt) = optional_text(r, name) {
        // Not empty-filtered: these are `Option`, so unset is already `None`, and an empty
        // string in one was written deliberately.
        return opt.clone();
    }
    derived_value(r, name)
}

/// The five fields every row carries. Always present, though the value may be empty.
fn required_text<'a>(r: &'a Issue, name: &str) -> Option<&'a str> {
    Some(match name {
        "id" => &r.id,
        "slug" => &r.slug,
        "title" => &r.title,
        "status" => &r.status,
        "priority" => &r.priority,
        _ => return None,
    })
}

/// The fields stored as an `Option`, where unset is the absence itself.
fn optional_text<'a>(r: &'a Issue, name: &str) -> Option<&'a Option<String>> {
    Some(match name {
        "parent" => &r.parent,
        "spec" => &r.spec,
        "review_url" => &r.review_url,
        "created" => &r.created,
        "started" => &r.started,
        "closed" => &r.closed,
        "resolution" => &r.resolution,
        _ => return None,
    })
}

/// The fields that are not stored as text — a count, two lists, a flag — and, failing all of
/// those, a custom field.
///
/// Each empty case is an absence for display: a `labels=[]` column says nothing, and
/// `manual_status=False` is the default every row would carry.
fn derived_value(r: &Issue, name: &str) -> Option<String> {
    match name {
        "points" => Some(r.points.to_string()),
        "labels" => (!r.labels.is_empty()).then(|| python_list(&r.labels)),
        "depends_on" => (!r.depends_on.is_empty()).then(|| python_list(&r.depends_on)),
        "manual_status" => r.manual_status.then(|| "True".to_string()),
        other => extra_value(r, other, false),
    }
}

/// A custom field's value. Non-string JSON is rendered as JSON, so an unknown key holding a
/// number or an object is still showable rather than blank.
///
/// `keep_empty` is the entire difference between the two entry points above.
fn extra_value(r: &Issue, name: &str, keep_empty: bool) -> Option<String> {
    match r.extra.get(name)? {
        Json::Null => None,
        Json::String(s) if s.is_empty() => keep_empty.then(String::new),
        Json::String(s) => Some(s.clone()),
        v => Some(v.to_json()),
    }
}

/// An empty string is an absence, for the readers that treat it as one.
fn non_empty(s: &str) -> Option<String> {
    (!s.is_empty()).then(|| s.to_string())
}

#[cfg(test)]
mod tests {
    // Tests assert; that is their job. The crate denies unwrap/expect/panic because a
    // malformed tracker must produce a diagnostic rather than a stack trace, but a test
    // that cannot panic cannot fail.
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    fn issue(extra: &[(&str, Json)]) -> Issue {
        let mut r = Issue {
            id: "aaaaaaa".into(),
            slug: "s".into(),
            title: "T".into(),
            status: "backlog".into(),
            priority: "high".into(),
            points: 3,
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
            extra: std::collections::BTreeMap::new(),
        };
        for (k, v) in extra {
            r.extra.insert((*k).to_string(), v.clone());
        }
        r
    }

    /// Every canonical key is readable, so `--show-field status` works as well as
    /// `--show-field assignee`. A key that fell through all three lookups would silently read
    /// as a *custom* field and answer `None` for every row.
    #[test]
    fn every_builtin_field_is_reachable() {
        let mut r = issue(&[]);
        r.parent = Some("p".into());
        r.labels = vec!["l".into()];
        r.depends_on = vec!["d".into()];
        r.spec = Some("sp".into());
        r.review_url = Some("ru".into());
        r.created = Some("c".into());
        r.started = Some("st".into());
        r.closed = Some("cl".into());
        r.resolution = Some("wontfix".into());
        r.manual_status = true;
        for key in CANON_KEYS {
            assert!(field_value(&r, key).is_some(), "{key} is not readable");
        }
    }

    /// The two lists and the flag render the way `show` and the conformance goldens expect —
    /// a Python list literal and `True`, not Rust's `["l"]` and `true`.
    #[test]
    fn lists_and_the_flag_render_pythons_way() {
        let mut r = issue(&[]);
        r.labels = vec!["a".into(), "b".into()];
        r.manual_status = true;
        assert_eq!(field_value(&r, "labels").as_deref(), Some("['a', 'b']"));
        assert_eq!(field_value(&r, "manual_status").as_deref(), Some("True"));
    }

    /// Empty is an absence for the display reader: a column of `labels=[]` on every row, or
    /// `manual_status=False`, carries no information.
    #[test]
    fn the_display_reader_drops_what_would_be_noise() {
        let r = issue(&[]);
        assert_eq!(field_value(&r, "labels"), None);
        assert_eq!(field_value(&r, "depends_on"), None);
        assert_eq!(field_value(&r, "manual_status"), None);
        assert_eq!(field_value(&r, "parent"), None);
        assert_eq!(field_value(&r, "points").as_deref(), Some("3"), "but a count always shows");
    }

    /// The one behavioural difference between the two entry points, both directions.
    #[test]
    fn an_empty_custom_value_is_a_value_only_to_the_raw_reader() {
        let r = issue(&[("note", Json::String(String::new()))]);
        assert_eq!(field_value_raw(&r, "note").as_deref(), Some(""), "--field note= set it");
        assert_eq!(field_value(&r, "note"), None, "a column of note= says nothing");
    }

    /// An empty *built-in* string is dropped by both, since neither reader has a way to have
    /// meant it — and `field_value_raw` defers to `field_value` for canonical keys.
    #[test]
    fn an_empty_builtin_is_dropped_by_both_readers() {
        let mut r = issue(&[]);
        r.title = String::new();
        assert_eq!(field_value(&r, "title"), None);
        assert_eq!(field_value_raw(&r, "title"), None);
    }

    #[test]
    fn a_custom_field_holding_non_string_json_is_still_showable() {
        let r = issue(&[("n", Json::Number("42".into())), ("o", Json::Object(vec![("k".into(), Json::Null)]))]);
        assert_eq!(field_value(&r, "n").as_deref(), Some("42"));
        assert_eq!(field_value(&r, "o").as_deref(), Some(r#"{"k": null}"#));
    }

    #[test]
    fn a_null_or_missing_custom_field_is_absent_to_both() {
        let r = issue(&[("z", Json::Null)]);
        for name in ["z", "never-set"] {
            assert_eq!(field_value(&r, name), None, "{name}");
            assert_eq!(field_value_raw(&r, name), None, "{name}");
        }
    }
}

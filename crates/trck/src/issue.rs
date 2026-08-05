//! The issue record, and the canonical form written to `index.jsonl`.
//!
//! The shape mirrors the Python engine exactly, because the conformance suite compares
//! `index.jsonl` byte for byte. Three things in particular are contract, not choice:
//!
//! * **Field order.** [`CANON_KEYS`] is the serialisation order, unknown keys after it.
//! * **Defaults are stripped.** A field equal to its default is omitted, so a tracker's
//!   diff shows what someone changed rather than what the writer happened to know.
//! * **Unknown keys survive.** [`Issue::extra`] round-trips anything this engine has
//!   never heard of. That is the guarantee the format version rests on: adding a field
//!   never makes an older engine *wrong*, only ignorant, so it needs no version bump.

use crate::json::Json;
use std::collections::BTreeMap;

/// The serialisation order. Exactly the known fields, nothing else.
pub(crate) const CANON_KEYS: &[&str] = &[
    "id",
    "slug",
    "title",
    "status",
    "priority",
    "points",
    "parent",
    "labels",
    "depends_on",
    "spec",
    "review_url",
    "created",
    "started",
    "closed",
    "resolution",
    "manual_status",
];

/// What `new` assigns when no weight is given.
pub(crate) const DEFAULT_POINTS: i64 = 1;

/// Index keys this engine rewrites on read. A custom field may not take one of these
/// names: it would be silently absorbed by the migration on the next load.
pub(crate) const LEGACY_KEYS: &[(&str, &str)] = &[
    ("milestone", "it migrates to a label; use `trck label`"),
    ("pr", "it migrates to `review_url`; use --review-url"),
];

/// A single issue.
///
/// The five identity/state fields are required; the rest carry defaults. Anything not
/// listed here lands in `extra` and is written back verbatim.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Issue {
    pub(crate) id: String,
    pub(crate) slug: String,
    pub(crate) title: String,
    pub(crate) status: String,
    pub(crate) priority: String,
    pub(crate) points: i64,
    pub(crate) parent: Option<String>,
    pub(crate) labels: Vec<String>,
    pub(crate) depends_on: Vec<String>,
    pub(crate) spec: Option<String>,
    pub(crate) review_url: Option<String>,
    pub(crate) created: Option<String>,
    pub(crate) started: Option<String>,
    pub(crate) closed: Option<String>,
    pub(crate) resolution: Option<String>,
    pub(crate) manual_status: bool,
    /// Unknown keys, verbatim. Sorted, so the canonical form is stable.
    pub(crate) extra: BTreeMap<String, Json>,
}

/// Validate a custom-field key.
///
/// Rejects the built-in names — use their flag or verb — and the legacy names, which
/// `Issue::from_json` migrates away on read: a custom field under one of those would be
/// swallowed the next time the index was loaded.
pub(crate) fn check_field_key(key: &str) -> Option<String> {
    if CANON_KEYS.contains(&key) {
        return Some(format!(
            "'{key}' is a built-in field; use its flag/verb, not --field/--unset"
        ));
    }
    if let Some((_, why)) = LEGACY_KEYS.iter().find(|(k, _)| *k == key) {
        return Some(format!(
            "'{key}' is a legacy field name ({why}), not a custom field"
        ));
    }
    if !is_field_key(key) {
        return Some(format!(
            "invalid field key '{key}' (must match [a-z][a-z0-9_-]*)"
        ));
    }
    None
}

fn is_field_key(key: &str) -> bool {
    let mut chars = key.chars();
    match chars.next() {
        Some(c) if c.is_ascii_lowercase() => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
}

/// How Python renders a value inside a diagnostic (`repr`), so the two engines' error
/// messages match. Only the shapes that reach an error path are covered.
fn py_repr(v: &Json) -> String {
    match v {
        Json::Null => "None".to_string(),
        Json::Bool(true) => "True".to_string(),
        Json::Bool(false) => "False".to_string(),
        Json::Number(raw) => raw.clone(),
        Json::String(s) => format!("'{}'", s.replace('\\', "\\\\").replace('\'', "\\'")),
        Json::Array(items) => {
            let inner: Vec<String> = items.iter().map(py_repr).collect();
            format!("[{}]", inner.join(", "))
        }
        Json::Object(pairs) => {
            let inner: Vec<String> = pairs
                .iter()
                .map(|(k, v)| format!("'{k}': {}", py_repr(v)))
                .collect();
            format!("{{{}}}", inner.join(", "))
        }
    }
}

fn bad(field: &str, msg: &str) -> String {
    format!("field '{field}' {msg}")
}

/// A row's key/value pairs during parsing, with later duplicates collapsed the way
/// Python's `json.loads` collapses them: the last one wins.
struct Row(Vec<(String, Json)>);

impl Row {
    fn new(pairs: &[(String, Json)]) -> Row {
        let mut out: Vec<(String, Json)> = Vec::new();
        for (k, v) in pairs {
            if let Some(slot) = out.iter_mut().find(|(existing, _)| existing == k) {
                slot.1 = v.clone();
            } else {
                out.push((k.clone(), v.clone()));
            }
        }
        Row(out)
    }

    fn get(&self, key: &str) -> Option<&Json> {
        self.0.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    /// The value, treating an explicit `null` as absent — which is what every optional
    /// field here means by it.
    fn present(&self, key: &str) -> Option<&Json> {
        match self.get(key) {
            Some(Json::Null) | None => None,
            v => v,
        }
    }

    fn take(&mut self, key: &str) -> Option<Json> {
        self.0
            .iter()
            .position(|(k, _)| k == key)
            .map(|i| self.0.remove(i).1)
    }

    fn set(&mut self, key: &str, value: Json) {
        self.0.push((key.to_string(), value));
    }

    fn into_extra(self) -> BTreeMap<String, Json> {
        self.0
            .into_iter()
            .filter(|(k, _)| !CANON_KEYS.contains(&k.as_str()))
            .collect()
    }
}

fn want_id(key: &str, v: &Json) -> Result<String, String> {
    match v {
        Json::String(s) if s.is_empty() => Err(bad(key, "must not be empty")),
        Json::String(s) => Ok(s.clone()),
        other => Err(bad(
            key,
            &format!("must be a string id, got {}", py_repr(other)),
        )),
    }
}

fn want_str(key: &str, v: &Json) -> Result<String, String> {
    v.as_str()
        .map(str::to_string)
        .ok_or_else(|| bad(key, &format!("must be a string, got {}", py_repr(v))))
}

fn opt_str(row: &Row, key: &str) -> Result<Option<String>, String> {
    row.present(key).map(|v| want_str(key, v)).transpose()
}

/// A list field, with each element checked by `element`. Absent or null is empty.
fn list_of(
    row: &Row,
    key: &str,
    element: fn(&str, &Json) -> Result<String, String>,
) -> Result<Vec<String>, String> {
    match row.present(key) {
        None => Ok(Vec::new()),
        Some(Json::Array(items)) => items.iter().map(|v| element(key, v)).collect(),
        Some(other) => Err(bad(key, &format!("must be a list, got {}", py_repr(other)))),
    }
}

fn want_label(key: &str, v: &Json) -> Result<String, String> {
    v.as_str().map(str::to_string).ok_or_else(|| {
        bad(
            key,
            &format!("must contain only strings, got {}", py_repr(v)),
        )
    })
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

impl Issue {
    /// Parse one index row.
    ///
    /// Loud and specific on failure: a row missing a required field or carrying a
    /// wrongly typed value is not a well-formed issue, and guessing at it is how a
    /// tracker silently loses data.
    pub(crate) fn from_json(raw: &Json) -> Result<Issue, String> {
        let Json::Object(pairs) = raw else {
            return Err(format!(
                "expected a JSON object, got {}",
                match raw {
                    Json::Array(_) => "list",
                    other => other.type_name(),
                }
            ));
        };
        let mut row = Row::new(pairs);
        let migrated_labels = migrate(&mut row);

        for key in ["id", "slug", "title", "status", "priority"] {
            if row.present(key).is_none() {
                return Err(bad(key, "is required"));
            }
        }
        let req = |key: &str| row.get(key).cloned().unwrap_or(Json::Null);
        let id = want_id("id", &req("id"))?;
        let slug = want_str("slug", &req("slug"))?;
        let title = want_str("title", &req("title"))?;
        let status = want_str("status", &req("status"))?;
        let priority = want_str("priority", &req("priority"))?;

        let points = match row.get("points") {
            None => DEFAULT_POINTS,
            Some(v) => v
                .as_i64()
                .ok_or_else(|| bad("points", &format!("must be an integer, got {}", py_repr(v))))?,
        };
        let parent = row
            .present("parent")
            .map(|v| want_id("parent", v))
            .transpose()?;

        let mut labels = list_of(&row, "labels", want_label)?;
        for name in migrated_labels {
            if !labels.contains(&name) {
                labels.push(name);
            }
        }
        let depends_on = list_of(&row, "depends_on", want_id)?;

        let manual_status = match row.get("manual_status") {
            None => false,
            Some(Json::Bool(b)) => *b,
            Some(other) => {
                return Err(bad(
                    "manual_status",
                    &format!("must be a boolean, got {}", py_repr(other)),
                ));
            }
        };

        Ok(Issue {
            id,
            slug,
            title,
            status,
            priority,
            points,
            parent,
            labels,
            depends_on,
            spec: opt_str(&row, "spec")?,
            review_url: opt_str(&row, "review_url")?,
            created: opt_str(&row, "created")?,
            started: opt_str(&row, "started")?,
            closed: opt_str(&row, "closed")?,
            resolution: opt_str(&row, "resolution")?,
            manual_status,
            extra: row.into_extra(),
        })
    }

    /// The full mapping every `--json` payload is built from: **every** canonical key in
    /// canonical order, `null` where unset, then the extras.
    ///
    /// The counterpart to [`to_canonical`](Self::to_canonical), and deliberately not the
    /// same shape. The index strips defaults because it is storage and a smaller diff is
    /// worth more there; a machine-readable payload keeps them, so a consumer can index a
    /// field without first checking whether this particular tracker happens to use it.
    pub(crate) fn to_full(&self) -> Json {
        let strings = |v: &[String]| Json::Array(v.iter().cloned().map(Json::String).collect());
        let s = |v: &str| Json::String(v.to_string());
        let opt = |v: &Option<String>| v.clone().map_or(Json::Null, Json::String);
        let mut out: Vec<(String, Json)> = vec![
            ("id".into(), s(&self.id)),
            ("slug".into(), s(&self.slug)),
            ("title".into(), s(&self.title)),
            ("status".into(), s(&self.status)),
            ("priority".into(), s(&self.priority)),
            ("points".into(), Json::Number(self.points.to_string())),
            ("parent".into(), opt(&self.parent)),
            ("labels".into(), strings(&self.labels)),
            ("depends_on".into(), strings(&self.depends_on)),
            ("spec".into(), opt(&self.spec)),
            ("review_url".into(), opt(&self.review_url)),
            ("created".into(), opt(&self.created)),
            ("started".into(), opt(&self.started)),
            ("closed".into(), opt(&self.closed)),
            ("resolution".into(), opt(&self.resolution)),
            ("manual_status".into(), Json::Bool(self.manual_status)),
        ];
        out.extend(self.extra.iter().map(|(k, v)| (k.clone(), v.clone())));
        Json::Object(out)
    }

    /// The slim, ordered form written to `index.jsonl`: known keys in canonical order
    /// with defaults stripped, then unknown keys in sorted order.
    pub(crate) fn to_canonical(&self) -> Json {
        let mut out: Vec<(String, Json)> = Vec::new();
        let mut put = |k: &str, v: Json| out.push((k.to_string(), v));
        let strings = |v: &[String]| Json::Array(v.iter().cloned().map(Json::String).collect());

        put("id", Json::String(self.id.clone()));
        put("slug", Json::String(self.slug.clone()));
        put("title", Json::String(self.title.clone()));
        put("status", Json::String(self.status.clone()));
        put("priority", Json::String(self.priority.clone()));
        if self.points != DEFAULT_POINTS {
            put("points", Json::Number(self.points.to_string()));
        }
        if let Some(v) = &self.parent {
            put("parent", Json::String(v.clone()));
        }
        if !self.labels.is_empty() {
            put("labels", strings(&self.labels));
        }
        if !self.depends_on.is_empty() {
            put("depends_on", strings(&self.depends_on));
        }
        for (key, value) in [
            ("spec", &self.spec),
            ("review_url", &self.review_url),
            ("created", &self.created),
            ("started", &self.started),
            ("closed", &self.closed),
            ("resolution", &self.resolution),
        ] {
            if let Some(v) = value {
                put(key, Json::String(v.clone()));
            }
        }
        if self.manual_status {
            put("manual_status", Json::Bool(true));
        }
        for (k, v) in &self.extra {
            put(k, v.clone());
        }
        Json::Object(out)
    }
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
            (
                r#""points": "lots""#,
                "field 'points' must be an integer, got 'lots'",
            ),
            (r#""labels": "x""#, "field 'labels' must be a list, got 'x'"),
            (
                r#""labels": [1]"#,
                "field 'labels' must contain only strings, got 1",
            ),
            (
                r#""manual_status": "yes""#,
                "field 'manual_status' must be a boolean, got 'yes'",
            ),
            (r#""slug": 3"#, "field 'slug' must be a string, got 3"),
        ];
        for (fragment, want) in cases {
            let raw = format!(
                r#"{{"id": "k3m9x2a", "slug": "s", "title": "T", "status": "backlog",
                     "priority": "high", {fragment}}}"#
            );
            let raw = raw.replace(
                r#""slug": "s", "#,
                if fragment.starts_with(r#""slug""#) {
                    ""
                } else {
                    r#""slug": "s", "#
                },
            );
            assert_eq!(issue(&raw).expect_err("should reject"), want);
        }
    }

    #[test]
    fn an_empty_id_is_rejected() {
        let raw = MINIMAL.replace("k3m9x2a", "");
        assert_eq!(
            issue(&raw).expect_err("should reject"),
            "field 'id' must not be empty"
        );
    }

    #[test]
    fn unknown_keys_round_trip_verbatim() {
        // The forward-compatibility guarantee the format version rests on.
        let raw = r#"{"id": "k3m9x2a", "slug": "s", "title": "T", "status": "backlog",
                     "priority": "high", "zeta": {"nested": [1, "two"]}}"#;
        let r = issue(raw).expect("parses");
        assert_eq!(
            r.extra.get("zeta").expect("kept").to_json(),
            r#"{"nested": [1, "two"]}"#
        );
        assert!(
            r.to_canonical()
                .to_json()
                .ends_with(r#""zeta": {"nested": [1, "two"]}}"#)
        );
    }

    #[test]
    fn canonical_order_is_the_field_order_then_extras_sorted() {
        let raw = r#"{"zeta": 1, "alpha": 2, "priority": "high", "status": "backlog",
                     "title": "T", "slug": "s", "id": "k3m9x2a"}"#;
        let out = issue(raw).expect("parses").to_canonical().to_json();
        assert_eq!(
            out,
            r#"{"id": "k3m9x2a", "slug": "s", "title": "T", "status": "backlog", "priority": "high", "alpha": 2, "zeta": 1}"#
        );
    }

    #[test]
    fn defaults_are_stripped() {
        let raw = r#"{"id": "k3m9x2a", "slug": "s", "title": "T", "status": "backlog",
                     "priority": "high", "points": 1, "labels": [], "parent": null,
                     "manual_status": false}"#;
        assert_eq!(
            issue(raw).expect("parses").to_canonical().to_json(),
            r#"{"id": "k3m9x2a", "slug": "s", "title": "T", "status": "backlog", "priority": "high"}"#
        );
    }

    #[test]
    fn a_non_default_value_is_kept() {
        // The test is equals-the-default, not falsiness: points 0 is meaningful.
        let raw = MINIMAL.replace(
            r#""priority": "high""#,
            r#""priority": "high", "points": 0"#,
        );
        assert!(
            issue(&raw)
                .expect("parses")
                .to_canonical()
                .to_json()
                .contains(r#""points": 0"#)
        );
    }

    #[test]
    fn milestone_migrates_to_a_label() {
        let raw = MINIMAL.replace(
            r#""priority": "high""#,
            r#""priority": "high", "milestone": "v1.0""#,
        );
        let r = issue(&raw).expect("parses");
        assert_eq!(r.labels, ["v1.0"]);
        assert!(!r.extra.contains_key("milestone"));
    }

    #[test]
    fn pr_migrates_to_review_url() {
        let raw = MINIMAL.replace(
            r#""priority": "high""#,
            r#""priority": "high", "pr": "https://example.test/pull/1""#,
        );
        let r = issue(&raw).expect("parses");
        assert_eq!(r.review_url.as_deref(), Some("https://example.test/pull/1"));
        assert!(!r.extra.contains_key("pr"));
        assert!(!r.to_canonical().to_json().contains("\"pr\""));
    }

    #[test]
    fn an_explicit_review_url_beats_a_stale_pr() {
        let raw = MINIMAL.replace(
            r#""priority": "high""#,
            r#""priority": "high", "pr": "https://old", "review_url": "https://new""#,
        );
        assert_eq!(
            issue(&raw).expect("parses").review_url.as_deref(),
            Some("https://new")
        );
    }

    #[test]
    fn legacy_names_are_not_available_as_custom_fields() {
        for (key, hint) in [("pr", "review_url"), ("milestone", "label")] {
            let msg = check_field_key(key).expect("should reject");
            assert!(msg.contains("legacy"), "{msg}");
            assert!(msg.contains(hint), "{msg}");
        }
    }

    #[test]
    fn built_in_names_are_not_available_as_custom_fields() {
        assert!(
            check_field_key("status")
                .expect("rejected")
                .contains("built-in")
        );
        assert!(check_field_key("Assignee").is_some());
        assert_eq!(check_field_key("assignee"), None);
    }

    #[test]
    fn a_non_object_row_is_an_error() {
        assert_eq!(
            Issue::from_json(&parse("[1]").expect("valid")).expect_err("rejected"),
            "expected a JSON object, got list"
        );
    }
}

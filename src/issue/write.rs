//! The two serialised forms, which are deliberately not the same shape.
//!
//! [`Issue::to_canonical`] is storage: known keys in [`super::CANON_KEYS`] order with defaults
//! stripped, so a tracker's diff shows what someone changed rather than what the writer
//! happened to know. [`Issue::to_full`] is a machine-readable payload: every key, `null` where
//! unset, so a consumer can index a field without first checking whether this particular
//! tracker happens to use it.

use super::{DEFAULT_POINTS, Issue};
use crate::json::Json;

/// A list of strings as a JSON array — the one conversion both forms need.
fn strings(v: &[String]) -> Json {
    Json::Array(v.iter().cloned().map(Json::String).collect())
}

impl Issue {
    /// The full mapping every `--json` payload is built from: **every** canonical key in
    /// canonical order, `null` where unset, then the extras.
    pub(crate) fn to_full(&self) -> Json {
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
        self.put_identity(&mut out);
        self.put_non_default(&mut out);
        out.extend(self.extra.iter().map(|(k, v)| (k.clone(), v.clone())));
        Json::Object(out)
    }

    /// The five fields every row carries. No default to compare against, so always written.
    fn put_identity(&self, out: &mut Vec<(String, Json)>) {
        for (key, value) in [("id", &self.id), ("slug", &self.slug), ("title", &self.title), ("status", &self.status), ("priority", &self.priority)] {
            out.push((key.to_string(), Json::String(value.clone())));
        }
    }

    /// Everything that has a default, written only where it differs from one.
    ///
    /// The test is equals-the-default, not falsiness: `points: 0` is a deliberate weight and
    /// is kept, where `points: 1` is the default and is not.
    fn put_non_default(&self, out: &mut Vec<(String, Json)>) {
        let mut put = |k: &str, v: Json| out.push((k.to_string(), v));
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
    }
}

#[cfg(test)]
mod tests {
    // Tests assert; that is their job. The crate denies unwrap/expect/panic because a
    // malformed tracker must produce a diagnostic rather than a stack trace, but a test
    // that cannot panic cannot fail.
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::super::CANON_KEYS;
    use super::*;
    use crate::json::parse;

    fn issue(text: &str) -> Result<Issue, String> {
        Issue::from_json(&parse(text).expect("valid JSON"))
    }

    const MINIMAL: &str = r#"{"id": "k3m9x2a", "slug": "s", "title": "T",
                              "status": "backlog", "priority": "high"}"#;

    #[test]
    fn unknown_keys_round_trip_verbatim() {
        // The forward-compatibility guarantee the format version rests on.
        let raw = r#"{"id": "k3m9x2a", "slug": "s", "title": "T", "status": "backlog",
                     "priority": "high", "zeta": {"nested": [1, "two"]}}"#;
        let r = issue(raw).expect("parses");
        assert_eq!(r.extra.get("zeta").expect("kept").to_json(), r#"{"nested": [1, "two"]}"#);
        assert!(r.to_canonical().to_json().ends_with(r#""zeta": {"nested": [1, "two"]}}"#));
    }

    #[test]
    fn canonical_order_is_the_field_order_then_extras_sorted() {
        let raw = r#"{"zeta": 1, "alpha": 2, "priority": "high", "status": "backlog",
                     "title": "T", "slug": "s", "id": "k3m9x2a"}"#;
        let out = issue(raw).expect("parses").to_canonical().to_json();
        assert_eq!(out, r#"{"id": "k3m9x2a", "slug": "s", "title": "T", "status": "backlog", "priority": "high", "alpha": 2, "zeta": 1}"#);
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

    /// Every optional field, present at once, in `CANON_KEYS` order — the order *is* the
    /// contract, and nothing else asserts the whole sequence.
    #[test]
    fn a_full_row_is_written_in_canonical_order() {
        let raw = r#"{"id": "a", "slug": "s", "title": "T", "status": "backlog",
                     "priority": "high", "points": 5, "parent": "p", "labels": ["l"],
                     "depends_on": ["d"], "spec": "sp", "review_url": "ru",
                     "created": "c", "started": "st", "closed": "cl",
                     "resolution": "wontfix", "manual_status": true, "zz": 1}"#;
        let out = issue(raw).expect("parses").to_canonical().to_json();
        let mut at = 0;
        for key in CANON_KEYS {
            let found = out.find(&format!("\"{key}\"")).unwrap_or_else(|| panic!("{key} missing from {out}"));
            assert!(found >= at, "{key} is out of canonical order in {out}");
            at = found;
        }
        assert!(out.ends_with(r#""zz": 1}"#), "extras come last: {out}");
    }

    /// `to_full` keeps what `to_canonical` strips — that is the whole difference between
    /// storage and a payload, and a consumer relies on the key being there.
    #[test]
    fn the_full_form_keeps_every_key_including_the_defaults() {
        let full = issue(MINIMAL).expect("parses").to_full();
        for key in CANON_KEYS {
            assert!(full.get(key).is_some(), "{key} missing from the full form");
        }
        assert_eq!(full.get("parent"), Some(&Json::Null), "unset is null, not absent");
        assert_eq!(full.get("labels"), Some(&Json::Array(Vec::new())));
        assert_eq!(full.get("manual_status"), Some(&Json::Bool(false)));
    }

    /// Both forms must survive a round-trip through the reader, or an index this engine wrote
    /// is one it cannot read.
    #[test]
    fn both_forms_read_back_to_the_same_issue() {
        let raw = r#"{"id": "a", "slug": "s", "title": "T", "status": "backlog",
                     "priority": "high", "points": 0, "labels": ["x"], "spec": "p",
                     "manual_status": true, "zz": [1, null]}"#;
        let original = issue(raw).expect("parses");
        assert_eq!(Issue::from_json(&original.to_canonical()).expect("canonical reads back"), original);
        assert_eq!(Issue::from_json(&original.to_full()).expect("full reads back"), original);
    }
}

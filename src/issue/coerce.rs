//! Turning a JSON value into the typed field it is supposed to be, or refusing.
//!
//! Refusing is the point. A row carrying a wrongly typed value is not a well-formed issue,
//! and guessing at it — a truthy `1` for a boolean, a stringified number for `points` — is how
//! a tracker silently loses what someone wrote.

use super::diagnostic::{bad, py_repr};
use super::row::Row;
use crate::json::Json;

/// An id: a non-empty string. Empty is called out separately from wrongly typed, because an
/// empty id is a plausible hand-edit and "must be a string" would be a confusing answer.
pub(super) fn want_id(key: &str, v: &Json) -> Result<String, String> {
    match v {
        Json::String(s) if s.is_empty() => Err(bad(key, "must not be empty")),
        Json::String(s) => Ok(s.clone()),
        other => Err(bad(key, &format!("must be a string id, got {}", py_repr(other)))),
    }
}

pub(super) fn want_str(key: &str, v: &Json) -> Result<String, String> {
    v.as_str().map(str::to_string).ok_or_else(|| bad(key, &format!("must be a string, got {}", py_repr(v))))
}

/// An optional string field. Absent and explicit `null` are the same answer.
pub(super) fn opt_str(row: &Row, key: &str) -> Result<Option<String>, String> {
    row.present(key).map(|v| want_str(key, v)).transpose()
}

/// A list field, with each element checked by `element`. Absent or null is empty.
///
/// The wrong *container* and a wrong *element* are different mistakes and say so differently:
/// `labels: "x"` is not a list, `labels: [1]` is a list of the wrong thing.
pub(super) fn list_of(row: &Row, key: &str, element: fn(&str, &Json) -> Result<String, String>) -> Result<Vec<String>, String> {
    match row.present(key) {
        None => Ok(Vec::new()),
        Some(Json::Array(items)) => items.iter().map(|v| element(key, v)).collect(),
        Some(other) => Err(bad(key, &format!("must be a list, got {}", py_repr(other)))),
    }
}

/// A label may be any non-empty string, so unlike an id it is only checked for being one.
pub(super) fn want_label(key: &str, v: &Json) -> Result<String, String> {
    v.as_str().map(str::to_string).ok_or_else(|| bad(key, &format!("must contain only strings, got {}", py_repr(v))))
}

#[cfg(test)]
mod tests {
    // Tests assert; that is their job. The crate denies unwrap/expect/panic because a
    // malformed tracker must produce a diagnostic rather than a stack trace, but a test
    // that cannot panic cannot fail.
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    fn row(pairs: &[(&str, Json)]) -> Row {
        Row::new(&pairs.iter().map(|(k, v)| ((*k).to_string(), v.clone())).collect::<Vec<_>>())
    }

    /// An empty id gets its own message. It is the likeliest hand-edit of the three, and
    /// "must be a string id" would be a confusing thing to tell someone who wrote `""`.
    #[test]
    fn an_id_is_checked_for_emptiness_separately_from_its_type() {
        assert_eq!(want_id("id", &Json::String(String::new())), Err("field 'id' must not be empty".into()));
        assert_eq!(want_id("id", &Json::Number("7".into())), Err("field 'id' must be a string id, got 7".into()));
        assert_eq!(want_id("id", &Json::String("a".into())), Ok("a".into()));
    }

    /// A list can be wrong two ways, and they are different mistakes.
    #[test]
    fn the_wrong_container_and_the_wrong_element_say_different_things() {
        let bad_container = row(&[("labels", Json::String("x".into()))]);
        assert_eq!(list_of(&bad_container, "labels", want_label), Err("field 'labels' must be a list, got 'x'".into()));
        let bad_element = row(&[("labels", Json::Array(vec![Json::Number("1".into())]))]);
        assert_eq!(list_of(&bad_element, "labels", want_label), Err("field 'labels' must contain only strings, got 1".into()));
    }

    #[test]
    fn an_absent_or_null_list_is_empty() {
        assert_eq!(list_of(&row(&[]), "labels", want_label), Ok(Vec::new()));
        assert_eq!(list_of(&row(&[("labels", Json::Null)]), "labels", want_label), Ok(Vec::new()));
    }

    #[test]
    fn an_absent_or_null_optional_string_is_none() {
        assert_eq!(opt_str(&row(&[]), "spec"), Ok(None));
        assert_eq!(opt_str(&row(&[("spec", Json::Null)]), "spec"), Ok(None));
        assert_eq!(opt_str(&row(&[("spec", Json::String("p".into()))]), "spec"), Ok(Some("p".into())));
    }

    /// `depends_on` reuses the id rule, so an empty string in the list is refused with the
    /// id's own wording rather than the label's.
    #[test]
    fn a_list_of_ids_enforces_the_id_rule_per_element() {
        let r = row(&[("depends_on", Json::Array(vec![Json::String(String::new())]))]);
        assert_eq!(list_of(&r, "depends_on", want_id), Err("field 'depends_on' must not be empty".into()));
    }
}

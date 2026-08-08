//! The wording of a rejected row.
//!
//! These messages are part of the contract, not decoration: the conformance suite compares
//! them literally, and the Python engine's phrasing is what they are compared against. So a
//! value quoted inside one is rendered the way Python's `repr` renders it, not the way Rust's
//! `Debug` would.

use crate::json::Json;

/// How Python renders a value inside a diagnostic (`repr`), so the two engines' error
/// messages match. Only the shapes that reach an error path are covered.
pub(super) fn py_repr(v: &Json) -> String {
    match v {
        Json::Null => "None".to_string(),
        Json::Bool(true) => "True".to_string(),
        Json::Bool(false) => "False".to_string(),
        Json::Number(raw) => raw.clone(),
        Json::String(s) => format!("'{}'", s.replace('\\', "\\\\").replace('\'', "\\'")),
        Json::Array(items) => {
            let inner: Vec<String> = items.iter().map(py_repr).collect();
            format!("[{}]", inner.join(", "))
        },
        Json::Object(pairs) => {
            let inner: Vec<String> = pairs.iter().map(|(k, v)| format!("'{k}': {}", py_repr(v))).collect();
            format!("{{{}}}", inner.join(", "))
        },
    }
}

/// Every field-level complaint has this shape, so the field is always named the same way.
pub(super) fn bad(field: &str, msg: &str) -> String {
    format!("field '{field}' {msg}")
}

#[cfg(test)]
mod tests {
    // Tests assert; that is their job. The crate denies unwrap/expect/panic because a
    // malformed tracker must produce a diagnostic rather than a stack trace, but a test
    // that cannot panic cannot fail.
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    /// Every shape, spelled the way Python spells it. `None`/`True`/`False` rather than
    /// `null`/`true`/`false`, and single-quoted strings — a Rust `Debug` would get all four
    /// wrong, and the conformance goldens compare these literally.
    #[test]
    fn values_are_rendered_the_way_python_reprs_them() {
        let cases = [
            (Json::Null, "None"),
            (Json::Bool(true), "True"),
            (Json::Bool(false), "False"),
            (Json::Number("1.5".into()), "1.5"),
            (Json::String("x".into()), "'x'"),
            (Json::Array(vec![Json::Number("1".into()), Json::String("t".into())]), "[1, 't']"),
            (Json::Object(vec![("k".into(), Json::Null)]), "{'k': None}"),
        ];
        for (v, want) in cases {
            assert_eq!(py_repr(&v), want);
        }
    }

    /// A quote or a backslash inside a string has to survive into the message, escaped the
    /// way Python escapes it — otherwise the diagnostic is unreadable exactly when the value
    /// is the problem.
    #[test]
    fn a_quote_or_backslash_in_a_string_is_escaped() {
        assert_eq!(py_repr(&Json::String("it's".into())), r"'it\'s'");
        assert_eq!(py_repr(&Json::String(r"a\b".into())), r"'a\\b'");
    }

    #[test]
    fn nesting_is_rendered_all_the_way_down() {
        let v = Json::Array(vec![Json::Object(vec![("a".into(), Json::Array(vec![Json::Bool(false)]))])]);
        assert_eq!(py_repr(&v), "[{'a': [False]}]");
    }

    #[test]
    fn a_field_complaint_names_the_field() {
        assert_eq!(bad("points", "must be an integer"), "field 'points' must be an integer");
    }
}

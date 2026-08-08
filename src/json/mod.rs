//! A JSON reader and writer, written out because the engine takes no dependencies.
//!
//! It is not a general-purpose library and does not try to be. It exists to read and
//! write `index.jsonl`, and the one property it must have is that a round-trip is
//! **byte-identical to what the Python engine produces** — `json.dumps(obj,
//! ensure_ascii=False)` with default separators. The conformance suite compares
//! `index.jsonl` literally, so a difference in escaping or spacing is a failure, not a
//! detail.
//!
//! Two decisions follow from that, and both are visible in [`Json`] itself:
//!
//! * **Objects keep insertion order** (a `Vec` of pairs, not a map). Python dicts do,
//!   and `to_canonical` relies on it to emit fields in the canonical order.
//! * **Numbers keep their source text.** Re-formatting a float is where two languages
//!   quietly disagree — `1e100` versus `1e+100` — and an unknown key carrying one has
//!   to survive a round-trip through an engine that does not understand it. Storing the
//!   token means an engine can preserve a number it never interprets.
//!
//! Reading is split three ways: [`cursor`] is the position and the primitives that move it,
//! [`parse`] the recursive shape, [`scan`] the two scalars with structure of their own.
//! [`render`] is the other direction, and the only place the byte-level contract lives.

mod cursor;
mod parse;
mod render;
mod scan;

pub(crate) use parse::parse;

/// A JSON value.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Json {
    Null,
    Bool(bool),
    /// The number's source text, preserved verbatim. See the module docs.
    Number(String),
    String(String),
    Array(Vec<Json>),
    /// Insertion-ordered, like a Python dict.
    Object(Vec<(String, Json)>),
}

impl Json {
    /// The value for `key`, or `None` when the value is not an object or has no such
    /// key. A later duplicate key wins, matching Python's `json.loads`.
    pub(crate) fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Object(pairs) => pairs.iter().rev().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    /// The value as an integer, or `None` when it is not a number or not integral.
    /// `true`/`false` are deliberately not integers here — Python's `isinstance(v, int)`
    /// accepts a bool, which is a trap the callers of this one do not want.
    pub(crate) fn as_i64(&self) -> Option<i64> {
        match self {
            Json::Number(raw) => raw.parse().ok(),
            _ => None,
        }
    }

    pub(crate) fn as_str(&self) -> Option<&str> {
        match self {
            Json::String(s) => Some(s),
            _ => None,
        }
    }

    /// The name of the value's type, for diagnostics.
    pub(crate) fn type_name(&self) -> &'static str {
        match self {
            Json::Null => "null",
            Json::Bool(_) => "bool",
            Json::Number(_) => "number",
            Json::String(_) => "string",
            Json::Array(_) => "array",
            Json::Object(_) => "object",
        }
    }
}

#[cfg(test)]
mod tests {
    // Tests assert; that is their job. The crate denies unwrap/expect/panic because a
    // malformed tracker must produce a diagnostic rather than a stack trace, but a test
    // that cannot panic cannot fail.
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    #[test]
    fn a_bool_is_not_an_integer() {
        assert_eq!(Json::Bool(true).as_i64(), None);
        assert_eq!(Json::Number("3".into()).as_i64(), Some(3));
    }

    /// Every accessor answers `None` rather than guessing when the value is the wrong shape:
    /// these read hand-editable rows, so the wrong shape is a case, not a bug.
    #[test]
    fn an_accessor_on_the_wrong_shape_declines() {
        let s = Json::String("x".into());
        assert_eq!(s.get("k"), None, "not an object");
        assert_eq!(s.as_i64(), None);
        assert_eq!(Json::Null.as_str(), None);
        assert_eq!(Json::Number("1.5".into()).as_i64(), None, "not integral");
    }

    #[test]
    fn type_name_covers_every_variant() {
        let all = [
            (Json::Null, "null"),
            (Json::Bool(false), "bool"),
            (Json::Number("1".into()), "number"),
            (Json::String(String::new()), "string"),
            (Json::Array(Vec::new()), "array"),
            (Json::Object(Vec::new()), "object"),
        ];
        for (v, want) in all {
            assert_eq!(v.type_name(), want);
        }
    }
}

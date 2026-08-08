//! Serialising, in the one form the engine is allowed to produce.
//!
//! The property that matters is byte-identity with `json.dumps(obj, ensure_ascii=False)` —
//! the conformance suite compares `index.jsonl` literally, so a difference in escaping or
//! spacing is a failure, not a detail.
//!
//! Indented output is **not** this encoder with whitespace inserted. Python drops the space
//! after `,` once `indent` is set, and keeps an empty container on one line rather than
//! splitting it across two. Both are reproduced, because a consumer diffing two engines'
//! output would read either as a difference.

use super::Json;
use std::fmt::Write as _;

impl Json {
    /// Serialise in the Python engine's canonical form: `", "` between items, `": "`
    /// after a key, non-ASCII left as-is (`ensure_ascii=False`).
    pub(crate) fn to_json(&self) -> String {
        let mut out = String::new();
        self.write(&mut out);
        out
    }

    /// The machine-readable rendering every `--json` path emits: Python's
    /// `json.dumps(obj, ensure_ascii=False, indent=2)`, byte for byte.
    pub(crate) fn to_json_pretty(&self) -> String {
        let mut out = String::new();
        self.write_pretty(0, &mut out);
        out
    }

    fn write(&self, out: &mut String) {
        match self {
            Json::Null => out.push_str("null"),
            Json::Bool(true) => out.push_str("true"),
            Json::Bool(false) => out.push_str("false"),
            Json::Number(raw) => out.push_str(raw),
            Json::String(s) => write_string(s, out),
            Json::Array(items) => Json::write_array(items, out),
            Json::Object(pairs) => Json::write_object(pairs, out),
        }
    }

    /// `[a, b]` — Python's default separators put a space after the comma.
    fn write_array(items: &[Json], out: &mut String) {
        out.push('[');
        for (i, item) in items.iter().enumerate() {
            if i > 0 {
                out.push_str(", ");
            }
            item.write(out);
        }
        out.push(']');
    }

    /// `{"k": v}`, in insertion order — which `to_canonical` relies on to emit fields in
    /// the canonical order.
    fn write_object(pairs: &[(String, Json)], out: &mut String) {
        out.push('{');
        for (i, (k, v)) in pairs.iter().enumerate() {
            if i > 0 {
                out.push_str(", ");
            }
            write_string(k, out);
            out.push_str(": ");
            v.write(out);
        }
        out.push('}');
    }

    fn write_pretty(&self, depth: usize, out: &mut String) {
        match self {
            Json::Array(items) if !items.is_empty() => {
                out.push_str("[\n");
                for (i, item) in items.iter().enumerate() {
                    Json::open_entry(i, depth + 1, out);
                    item.write_pretty(depth + 1, out);
                }
                Json::close_block(']', depth, out);
            },
            Json::Object(pairs) if !pairs.is_empty() => {
                out.push_str("{\n");
                for (i, (k, v)) in pairs.iter().enumerate() {
                    Json::open_entry(i, depth + 1, out);
                    write_string(k, out);
                    out.push_str(": ");
                    v.write_pretty(depth + 1, out);
                }
                Json::close_block('}', depth, out);
            },
            // Scalars, and the empty containers, render the same either way.
            other => other.write(out),
        }
    }

    /// The comma-newline before every entry but the first, then that entry's indent. No
    /// space after the comma: `indent` turns Python's item separator into a bare `,`.
    fn open_entry(i: usize, depth: usize, out: &mut String) {
        if i > 0 {
            out.push_str(",\n");
        }
        out.push_str(&"  ".repeat(depth));
    }

    /// The newline, the closing bracket's own indent, and the bracket.
    fn close_block(bracket: char, depth: usize, out: &mut String) {
        out.push('\n');
        out.push_str(&"  ".repeat(depth));
        out.push(bracket);
    }
}

/// The short escape Python uses for a character, if it has one.
///
/// The inverse of [`super::scan`]'s table, minus `/`: reading `\/` is required, writing it is
/// optional, and Python does not.
fn short_escape(c: char) -> Option<&'static str> {
    Some(match c {
        '"' => "\\\"",
        '\\' => "\\\\",
        '\n' => "\\n",
        '\r' => "\\r",
        '\t' => "\\t",
        '\u{8}' => "\\b",
        '\u{c}' => "\\f",
        _ => return None,
    })
}

/// Escape exactly what Python's encoder escapes with `ensure_ascii=False`: the two
/// structural characters, the five short forms, and any other C0 control as `\uXXXX`.
/// Notably `/` and DEL are *not* escaped — escaping them would be valid JSON and a
/// byte-level mismatch.
fn write_string(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        if let Some(esc) = short_escape(c) {
            out.push_str(esc);
        } else if (c as u32) < 0x20 {
            let _ = write!(out, "\\u{:04x}", c as u32);
        } else {
            out.push(c);
        }
    }
    out.push('"');
}

#[cfg(test)]
mod tests {
    // Tests assert; that is their job. The crate denies unwrap/expect/panic because a
    // malformed tracker must produce a diagnostic rather than a stack trace, but a test
    // that cannot panic cannot fail.
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    fn parse(text: &str) -> Result<Json, String> {
        crate::json::parse(text)
    }

    fn roundtrip(text: &str) -> String {
        parse(text).expect("parses").to_json()
    }

    /// `(canonical, json.dumps(..., indent=2))` — every string generated by `CPython`, not
    /// written by hand.
    ///
    /// Each source is already a **fixed point of Python's encoder**, which is what any
    /// `index.jsonl` written by an engine is. That matters: Python's own round-trip is lossy
    /// for numbers (`json.loads("-0")` is `0`, `1E+2` is `100.0`) where this engine preserves
    /// the source text on purpose — see `preserves_number_text` in `scan`. Comparing against
    /// non-canonical input would be asserting the divergence backwards.
    ///
    /// What is left is the module's whole byte-level contract, and the part a
    /// reimplementation gets subtly wrong: the space after `,` that `indent` removes, which
    /// characters escape, `/` and DEL left alone, non-ASCII passed through.
    const PYTHON: &[(&str, &str)] = &[
        ("{}", "{}"),
        ("[]", "[]"),
        ("null", "null"),
        ("true", "true"),
        ("false", "false"),
        ("0", "0"),
        ("42", "42"),
        ("-17", "-17"),
        ("1.5", "1.5"),
        ("1e+100", "1e+100"),
        ("0.0", "0.0"),
        ("1e-07", "1e-07"),
        ("\"plain\"", "\"plain\""),
        ("\"café ✓\"", "\"café ✓\""),
        ("\"emoji 😀\"", "\"emoji 😀\""),
        ("\"tab\\there\"", "\"tab\\there\""),
        ("\"nl\\nhere\"", "\"nl\\nhere\""),
        ("\"quote\\\"q\"", "\"quote\\\"q\""),
        ("\"back\\\\slash\"", "\"back\\\\slash\""),
        ("\"slash/none\"", "\"slash/none\""),
        ("\"\\u0001\\u001f\"", "\"\\u0001\\u001f\""),
        ("\"\"", "\"\""),
        ("\"😀\"", "\"😀\""),
        ("\"delend\"", "\"delend\""),
        ("\"\\b\\f\\r\"", "\"\\b\\f\\r\""),
        ("{\"a\": 1, \"b\": [1, 2], \"c\": {\"d\": null}}", "{\n  \"a\": 1,\n  \"b\": [\n    1,\n    2\n  ],\n  \"c\": {\n    \"d\": null\n  }\n}"),
        ("{\"z\": 1, \"a\": 2}", "{\n  \"z\": 1,\n  \"a\": 2\n}"),
        ("[[[[1]]]]", "[\n  [\n    [\n      [\n        1\n      ]\n    ]\n  ]\n]"),
        ("{\"k\": [], \"j\": {}}", "{\n  \"k\": [],\n  \"j\": {}\n}"),
        ("[1, \"two\", null, true, false, 2.5]", "[\n  1,\n  \"two\",\n  null,\n  true,\n  false,\n  2.5\n]"),
        ("{\"unicode key ✓\": \"v\"}", "{\n  \"unicode key ✓\": \"v\"\n}"),
        ("{\"deep\": {\"a\": [{\"b\": []}, {}]}}", "{\n  \"deep\": {\n    \"a\": [\n      {\n        \"b\": []\n      },\n      {}\n    ]\n  }\n}"),
    ];

    #[test]
    fn matches_cpython_byte_for_byte() {
        for (canonical, pretty) in PYTHON {
            let v = parse(canonical).unwrap_or_else(|e| panic!("{canonical}: {e}"));
            assert_eq!(&v.to_json(), canonical, "compact form of {canonical}");
            assert_eq!(&v.to_json_pretty(), pretty, "indented form of {canonical}");
        }
    }

    /// Re-reading our own output must give the same value back — for both forms, so the
    /// indented encoder is not quietly emitting something only Python can read.
    #[test]
    fn our_own_output_parses_back_to_the_same_value() {
        for (canonical, _) in PYTHON {
            let v = parse(canonical).expect("parses");
            assert_eq!(parse(&v.to_json()).expect("compact reparses"), v, "compact {canonical}");
            assert_eq!(parse(&v.to_json_pretty()).expect("pretty reparses"), v, "pretty {canonical}");
        }
    }

    #[test]
    fn pretty_matches_pythons_indent_two() {
        // Verbatim from `json.dumps(obj, ensure_ascii=False, indent=2)`: no space after a
        // comma, and empty containers stay on one line instead of splitting.
        let doc = parse(
            r#"{"id": "a", "labels": [], "extra": {}, "depends_on": ["x", "y"],
                "nested": {"k": 1}, "n": null, "b": true}"#,
        )
        .expect("parses");
        assert_eq!(
            doc.to_json_pretty(),
            "{\n  \"id\": \"a\",\n  \"labels\": [],\n  \"extra\": {},\n  \"depends_on\": [\n    \
             \"x\",\n    \"y\"\n  ],\n  \"nested\": {\n    \"k\": 1\n  },\n  \"n\": null,\n  \
             \"b\": true\n}"
        );
    }

    #[test]
    fn pretty_leaves_a_bare_scalar_alone() {
        assert_eq!(parse("42").expect("parses").to_json_pretty(), "42");
        assert_eq!(parse("[]").expect("parses").to_json_pretty(), "[]");
    }

    #[test]
    fn writes_python_separators() {
        assert_eq!(roundtrip(r#"{"a":1,"b":[1,2]}"#), r#"{"a": 1, "b": [1, 2]}"#);
    }

    #[test]
    fn keeps_object_order() {
        assert_eq!(roundtrip(r#"{"z":1,"a":2}"#), r#"{"z": 1, "a": 2}"#);
    }

    #[test]
    fn leaves_non_ascii_alone() {
        // ensure_ascii=False. Escaping it would be valid JSON and a byte mismatch.
        assert_eq!(roundtrip(r#"{"t":"café ✓"}"#), r#"{"t": "café ✓"}"#);
    }

    #[test]
    fn escapes_what_python_escapes_and_no_more() {
        let v = Json::String("a\"b\\c\nd\te/f\u{7f}\u{1}".to_string());
        assert_eq!(v.to_json(), "\"a\\\"b\\\\c\\nd\\te/f\u{7f}\\u0001\"");
    }

    #[test]
    fn empty_containers_round_trip() {
        assert_eq!(roundtrip("{}"), "{}");
        assert_eq!(roundtrip("[]"), "[]");
    }
}

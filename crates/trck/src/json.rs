//! A JSON reader and writer, written out because the engine takes no dependencies.
//!
//! It is not a general-purpose library and does not try to be. It exists to read and
//! write `index.jsonl`, and the one property it must have is that a round-trip is
//! **byte-identical to what the Python engine produces** — `json.dumps(obj,
//! ensure_ascii=False)` with default separators. The conformance suite compares
//! `index.jsonl` literally, so a difference in escaping or spacing is a failure, not a
//! detail.
//!
//! Two decisions follow from that:
//!
//! * **Objects keep insertion order** (a `Vec` of pairs, not a map). Python dicts do,
//!   and `to_canonical` relies on it to emit fields in the canonical order.
//! * **Numbers keep their source text.** Re-formatting a float is where two languages
//!   quietly disagree — `1e100` versus `1e+100` — and an unknown key carrying one has
//!   to survive a round-trip through an engine that does not understand it. Storing the
//!   token means an engine can preserve a number it never interprets.

use std::fmt::Write as _;

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

    /// Serialise in the Python engine's canonical form: `", "` between items, `": "`
    /// after a key, non-ASCII left as-is (`ensure_ascii=False`).
    pub(crate) fn to_json(&self) -> String {
        let mut out = String::new();
        self.write(&mut out);
        out
    }

    /// The machine-readable rendering every `--json` path emits: Python's
    /// `json.dumps(obj, ensure_ascii=False, indent=2)`, byte for byte.
    ///
    /// Indented output is not the same encoder with whitespace inserted — Python drops the
    /// space after `,` once `indent` is set, and keeps an empty container on one line
    /// rather than splitting it across two. Both are reproduced here, because a consumer
    /// diffing two engines' output would see either as a difference.
    pub(crate) fn to_json_pretty(&self) -> String {
        let mut out = String::new();
        self.write_pretty(0, &mut out);
        out
    }

    fn write_pretty(&self, depth: usize, out: &mut String) {
        let pad = |n: usize, out: &mut String| out.push_str(&"  ".repeat(n));
        match self {
            Json::Array(items) if !items.is_empty() => {
                out.push_str("[\n");
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        out.push_str(",\n");
                    }
                    pad(depth + 1, out);
                    item.write_pretty(depth + 1, out);
                }
                out.push('\n');
                pad(depth, out);
                out.push(']');
            },
            Json::Object(pairs) if !pairs.is_empty() => {
                out.push_str("{\n");
                for (i, (k, v)) in pairs.iter().enumerate() {
                    if i > 0 {
                        out.push_str(",\n");
                    }
                    pad(depth + 1, out);
                    write_string(k, out);
                    out.push_str(": ");
                    v.write_pretty(depth + 1, out);
                }
                out.push('\n');
                pad(depth, out);
                out.push('}');
            },
            // Scalars, and the empty containers, render the same either way.
            other => other.write(out),
        }
    }

    fn write(&self, out: &mut String) {
        match self {
            Json::Null => out.push_str("null"),
            Json::Bool(true) => out.push_str("true"),
            Json::Bool(false) => out.push_str("false"),
            Json::Number(raw) => out.push_str(raw),
            Json::String(s) => write_string(s, out),
            Json::Array(items) => {
                out.push('[');
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    item.write(out);
                }
                out.push(']');
            },
            Json::Object(pairs) => {
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
            },
        }
    }
}

/// Escape exactly what Python's encoder escapes with `ensure_ascii=False`: the two
/// structural characters, the five short forms, and any other C0 control as `\uXXXX`.
/// Notably `/` and DEL are *not* escaped — escaping them would be valid JSON and a
/// byte-level mismatch.
fn write_string(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            },
            c => out.push(c),
        }
    }
    out.push('"');
}

/// Parse one JSON document. Trailing content is an error, as it is in Python.
pub(crate) fn parse(text: &str) -> Result<Json, String> {
    let mut p = Parser { chars: text.chars().collect(), pos: 0 };
    p.skip_ws();
    let value = p.value()?;
    p.skip_ws();
    if p.pos < p.chars.len() {
        return Err(format!("trailing data at position {}", p.pos));
    }
    Ok(value)
}

struct Parser {
    chars: Vec<char>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.peek();
        if c.is_some() {
            self.pos += 1;
        }
        c
    }

    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(' ' | '\t' | '\n' | '\r')) {
            self.pos += 1;
        }
    }

    fn expect(&mut self, want: char) -> Result<(), String> {
        match self.bump() {
            Some(c) if c == want => Ok(()),
            Some(c) => Err(format!("expected '{want}' at position {}, got '{c}'", self.pos - 1)),
            None => Err(format!("expected '{want}', got end of input")),
        }
    }

    fn literal(&mut self, word: &str, value: Json) -> Result<Json, String> {
        for want in word.chars() {
            self.expect(want)?;
        }
        Ok(value)
    }

    fn value(&mut self) -> Result<Json, String> {
        match self.peek() {
            Some('{') => self.object(),
            Some('[') => self.array(),
            Some('"') => Ok(Json::String(self.string()?)),
            Some('t') => self.literal("true", Json::Bool(true)),
            Some('f') => self.literal("false", Json::Bool(false)),
            Some('n') => self.literal("null", Json::Null),
            Some(c) if c == '-' || c.is_ascii_digit() => self.number(),
            Some(c) => Err(format!("unexpected '{c}' at position {}", self.pos)),
            None => Err("unexpected end of input".to_string()),
        }
    }

    fn object(&mut self) -> Result<Json, String> {
        self.expect('{')?;
        let mut pairs = Vec::new();
        self.skip_ws();
        if self.peek() == Some('}') {
            self.pos += 1;
            return Ok(Json::Object(pairs));
        }
        loop {
            self.skip_ws();
            let key = self.string()?;
            self.skip_ws();
            self.expect(':')?;
            self.skip_ws();
            pairs.push((key, self.value()?));
            self.skip_ws();
            match self.bump() {
                Some(',') => {},
                Some('}') => return Ok(Json::Object(pairs)),
                Some(c) => return Err(format!("expected ',' or '}}', got '{c}'")),
                None => return Err("unterminated object".to_string()),
            }
        }
    }

    fn array(&mut self) -> Result<Json, String> {
        self.expect('[')?;
        let mut items = Vec::new();
        self.skip_ws();
        if self.peek() == Some(']') {
            self.pos += 1;
            return Ok(Json::Array(items));
        }
        loop {
            self.skip_ws();
            items.push(self.value()?);
            self.skip_ws();
            match self.bump() {
                Some(',') => {},
                Some(']') => return Ok(Json::Array(items)),
                Some(c) => return Err(format!("expected ',' or ']', got '{c}'")),
                None => return Err("unterminated array".to_string()),
            }
        }
    }

    fn string(&mut self) -> Result<String, String> {
        self.expect('"')?;
        let mut out = String::new();
        loop {
            match self.bump() {
                None => return Err("unterminated string".to_string()),
                Some('"') => return Ok(out),
                Some('\\') => match self.bump() {
                    Some('"') => out.push('"'),
                    Some('\\') => out.push('\\'),
                    Some('/') => out.push('/'),
                    Some('b') => out.push('\u{8}'),
                    Some('f') => out.push('\u{c}'),
                    Some('n') => out.push('\n'),
                    Some('r') => out.push('\r'),
                    Some('t') => out.push('\t'),
                    Some('u') => out.push(self.unicode_escape()?),
                    Some(c) => return Err(format!("invalid escape '\\{c}'")),
                    None => return Err("unterminated escape".to_string()),
                },
                Some(c) => out.push(c),
            }
        }
    }

    /// A `\uXXXX` escape, joining a surrogate pair when one follows. Lone surrogates
    /// become U+FFFD rather than an error, matching Python's tolerance — a malformed
    /// title should not make a tracker unreadable.
    fn unicode_escape(&mut self) -> Result<char, String> {
        let high = self.hex4()?;
        if (0xD800..0xDC00).contains(&high) {
            let save = self.pos;
            if self.peek() == Some('\\') {
                self.pos += 1;
                if self.peek() == Some('u') {
                    self.pos += 1;
                    let low = self.hex4()?;
                    if (0xDC00..0xE000).contains(&low) {
                        let combined = 0x1_0000 + ((high - 0xD800) << 10) + (low - 0xDC00);
                        return char::from_u32(combined).ok_or_else(|| "bad surrogate pair".into());
                    }
                }
            }
            self.pos = save;
        }
        Ok(char::from_u32(high).unwrap_or('\u{FFFD}'))
    }

    fn hex4(&mut self) -> Result<u32, String> {
        let mut n = 0u32;
        for _ in 0..4 {
            let c = self.bump().ok_or("truncated \\u escape")?;
            let d = c.to_digit(16).ok_or_else(|| format!("bad hex digit '{c}'"))?;
            n = n * 16 + d;
        }
        Ok(n)
    }

    fn number(&mut self) -> Result<Json, String> {
        let start = self.pos;
        if self.peek() == Some('-') {
            self.pos += 1;
        }
        let digits = |p: &mut Self| {
            let from = p.pos;
            while matches!(p.peek(), Some(c) if c.is_ascii_digit()) {
                p.pos += 1;
            }
            p.pos > from
        };
        // JSON forbids a leading zero (`01`), and so does Python's decoder. Accepting
        // it would make a malformed index parse here and fail there.
        match self.peek() {
            Some('0') => {
                self.pos += 1;
                if matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
                    return Err(format!("leading zero at position {start}"));
                }
            },
            Some(c) if c.is_ascii_digit() => {
                digits(self);
            },
            _ => return Err(format!("expected a digit at position {}", self.pos)),
        }
        if self.peek() == Some('.') {
            self.pos += 1;
            if !digits(self) {
                return Err("expected a digit after '.'".to_string());
            }
        }
        if matches!(self.peek(), Some('e' | 'E')) {
            self.pos += 1;
            if matches!(self.peek(), Some('+' | '-')) {
                self.pos += 1;
            }
            if !digits(self) {
                return Err("expected a digit in the exponent".to_string());
            }
        }
        Ok(Json::Number(self.chars[start..self.pos].iter().collect()))
    }
}

#[cfg(test)]
mod tests {
    // Tests assert; that is their job. The crate denies unwrap/expect/panic because a
    // malformed tracker must produce a diagnostic rather than a stack trace, but a test
    // that cannot panic cannot fail.
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    fn roundtrip(text: &str) -> String {
        parse(text).expect("parses").to_json()
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
    fn preserves_number_text() {
        // Re-formatting is where two languages quietly disagree; an engine that does
        // not interpret a number should still hand it back unchanged.
        for n in ["1", "-0", "1.50", "1e100", "1E+2", "0.0"] {
            assert_eq!(roundtrip(&format!("[{n}]")), format!("[{n}]"));
        }
    }

    #[test]
    fn reads_escapes_and_surrogate_pairs() {
        assert_eq!(parse(r#""A😀\t""#).expect("parses"), Json::String("A\u{1F600}\t".to_string()));
    }

    #[test]
    fn rejects_malformed_input() {
        for bad in [r#"{"a": }"#, "[1,", r#""unterminated"#, "{}{}", "01", "tru"] {
            assert!(parse(bad).is_err(), "should reject {bad}");
        }
    }

    #[test]
    fn a_bool_is_not_an_integer() {
        assert_eq!(Json::Bool(true).as_i64(), None);
        assert_eq!(Json::Number("3".into()).as_i64(), Some(3));
    }

    #[test]
    fn empty_containers_round_trip() {
        assert_eq!(roundtrip("{}"), "{}");
        assert_eq!(roundtrip("[]"), "[]");
    }
}

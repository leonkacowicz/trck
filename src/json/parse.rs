//! The structural half of the parser: which value comes next, and where one ends.
//!
//! Scalars are scanned in [`super::scan`]. What is left here is the recursive shape —
//! objects, arrays, and the comma-or-closer decision both of them make.

use super::Json;
use super::cursor::Parser;

/// Parse one JSON document. Trailing content is an error, as it is in Python.
pub(crate) fn parse(text: &str) -> Result<Json, String> {
    let mut p = Parser::new(text);
    p.skip_ws();
    let value = p.value()?;
    p.skip_ws();
    if p.pos < p.chars.len() {
        return Err(format!("trailing data at position {}", p.pos));
    }
    Ok(value)
}

impl Parser {
    pub(super) fn value(&mut self) -> Result<Json, String> {
        match self.peek() {
            Some('{') => self.object(),
            Some('[') => self.array(),
            Some('"') => Ok(Json::String(self.string()?)),
            Some(c) if c == '-' || c.is_ascii_digit() => self.number(),
            Some(c) => self.keyword(c),
            None => Err("unexpected end of input".to_string()),
        }
    }

    /// The three bare words JSON allows where a value is expected. Anything else starting a
    /// value is simply not one, and this is where that is said.
    fn keyword(&mut self, c: char) -> Result<Json, String> {
        match c {
            't' => self.literal("true", Json::Bool(true)),
            'f' => self.literal("false", Json::Bool(false)),
            'n' => self.literal("null", Json::Null),
            _ => Err(format!("unexpected '{c}' at position {}", self.pos)),
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
            if !self.more('}', "object")? {
                return Ok(Json::Object(pairs));
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
            if !self.more(']', "array")? {
                return Ok(Json::Array(items));
            }
        }
    }

    /// After an element: `,` to continue, the closing bracket to stop. `Ok(true)` means
    /// another element follows.
    ///
    /// Both containers make exactly this decision, and getting it wrong in one but not the
    /// other is the kind of divergence nobody notices until a file will not load.
    fn more(&mut self, close: char, what: &str) -> Result<bool, String> {
        self.skip_ws();
        match self.bump() {
            Some(',') => Ok(true),
            Some(c) if c == close => Ok(false),
            Some(c) => Err(format!("expected ',' or '{close}', got '{c}'")),
            None => Err(format!("unterminated {what}")),
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
    fn rejects_malformed_input() {
        for bad in [r#"{"a": }"#, "[1,", r#""unterminated"#, "{}{}", "01", "tru"] {
            assert!(parse(bad).is_err(), "should reject {bad}");
        }
    }

    /// Every structural way a container can be wrong, each naming what it was reading — the
    /// two containers share one implementation of this decision, so both are checked.
    #[test]
    fn an_unclosed_container_says_which_one() {
        assert!(parse("{\"a\": 1").expect_err("unclosed object").contains("object"));
        assert!(parse("[1").expect_err("unclosed array").contains("array"));
        assert!(parse("{\"a\" 1}").expect_err("missing colon").contains("':'"));
        assert!(parse("[1 2]").expect_err("missing comma").contains("','"));
        assert!(parse("{1: 2}").is_err(), "a non-string key is not a key");
    }

    #[test]
    fn whitespace_is_allowed_wherever_json_allows_it() {
        let spaced = " {\n\t\"a\" : [ 1 , 2 ] ,\r\n \"b\" : { } } ";
        assert_eq!(parse(spaced).expect("parses").to_json(), r#"{"a": [1, 2], "b": {}}"#);
    }

    #[test]
    fn nesting_survives_several_levels() {
        assert_eq!(parse("[[[[1]]]]").expect("parses").to_json(), "[[[[1]]]]");
    }

    /// A later duplicate wins, matching Python's `json.loads` — but both pairs are kept, so
    /// a round-trip does not silently drop one.
    #[test]
    fn a_duplicate_key_resolves_to_the_last_one() {
        let v = parse(r#"{"dup": 1, "dup": 2}"#).expect("parses");
        assert_eq!(v.get("dup").and_then(Json::as_i64), Some(2));
    }
}

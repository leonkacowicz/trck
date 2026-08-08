//! Scanning the two scalars with internal structure: strings and numbers.
//!
//! A number is **not converted**. Its source text is kept verbatim, because re-formatting a
//! float is exactly where two languages quietly disagree — `1e100` against `1e+100` — and an
//! unknown key carrying one has to survive a round-trip through an engine that never
//! interprets it. So the scanners here only decide where a token *ends*.

use super::Json;
use super::cursor::Parser;

/// The eight single-character escapes JSON defines, and the character each stands for.
///
/// The inverse of [`super::render::short_escape`], minus `/`: reading `\/` is required,
/// writing it is optional, and Python does not — so this table is the longer of the two.
const ESCAPES: [(char, char); 8] = [('"', '"'), ('\\', '\\'), ('/', '/'), ('b', '\u{8}'), ('f', '\u{c}'), ('n', '\n'), ('r', '\r'), ('t', '\t')];

impl Parser {
    pub(super) fn string(&mut self) -> Result<String, String> {
        self.expect('"')?;
        let mut out = String::new();
        loop {
            match self.bump() {
                None => return Err("unterminated string".to_string()),
                Some('"') => return Ok(out),
                Some('\\') => out.push(self.escape()?),
                Some(c) => out.push(c),
            }
        }
    }

    /// One escape sequence, the backslash already consumed.
    fn escape(&mut self) -> Result<char, String> {
        match self.bump() {
            None => Err("unterminated escape".to_string()),
            Some('u') => self.unicode_escape(),
            Some(c) => ESCAPES.iter().find(|(from, _)| *from == c).map(|(_, to)| *to).ok_or_else(|| format!("invalid escape '\\{c}'")),
        }
    }

    /// A `\uXXXX` escape, joining a surrogate pair when one follows. Lone surrogates
    /// become U+FFFD rather than an error, matching Python's tolerance — a malformed
    /// title should not make a tracker unreadable.
    fn unicode_escape(&mut self) -> Result<char, String> {
        let high = self.hex4()?;
        if (0xD800..0xDC00).contains(&high) {
            let save = self.pos;
            // The arithmetic lands in `0x10000..=0x10FFFF` for every pair in range, which
            // holds no surrogates — so `from_u32` cannot fail here, and there is no
            // bad-pair error to report.
            if let Some(low) = self.trailing_surrogate()?
                && let Some(joined) = char::from_u32(0x1_0000 + ((high - 0xD800) << 10) + (low - 0xDC00))
            {
                return Ok(joined);
            }
            // Not a pair after all: give back whatever the attempt consumed.
            self.pos = save;
        }
        Ok(char::from_u32(high).unwrap_or('\u{FFFD}'))
    }

    /// The low half of a surrogate pair, if `\uDCxx`–`\uDFxx` is what comes next.
    fn trailing_surrogate(&mut self) -> Result<Option<u32>, String> {
        if self.peek() != Some('\\') {
            return Ok(None);
        }
        self.pos += 1;
        if self.peek() != Some('u') {
            return Ok(None);
        }
        self.pos += 1;
        let low = self.hex4()?;
        Ok((0xDC00..0xE000).contains(&low).then_some(low))
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

    /// A number token, returned as the text it was written as.
    pub(super) fn number(&mut self) -> Result<Json, String> {
        let start = self.pos;
        if self.peek() == Some('-') {
            self.pos += 1;
        }
        self.int_part(start)?;
        self.frac_part()?;
        self.exp_part()?;
        Ok(Json::Number(self.chars[start..self.pos].iter().collect()))
    }

    /// Consume a run of digits, reporting whether there were any.
    fn digits(&mut self) -> bool {
        let from = self.pos;
        while matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
            self.pos += 1;
        }
        self.pos > from
    }

    /// The mandatory integer part. JSON forbids a leading zero (`01`), and so does Python's
    /// decoder: accepting it would make a malformed index parse here and fail there.
    fn int_part(&mut self, start: usize) -> Result<(), String> {
        match self.peek() {
            Some('0') => {
                self.pos += 1;
                if matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
                    return Err(format!("leading zero at position {start}"));
                }
            },
            Some(c) if c.is_ascii_digit() => {
                self.digits();
            },
            _ => return Err(format!("expected a digit at position {}", self.pos)),
        }
        Ok(())
    }

    /// An optional `.ddd`. The dot commits: a digit must follow.
    fn frac_part(&mut self) -> Result<(), String> {
        if self.peek() == Some('.') {
            self.pos += 1;
            if !self.digits() {
                return Err("expected a digit after '.'".to_string());
            }
        }
        Ok(())
    }

    /// An optional `e[+-]ddd`. The `e` commits in the same way.
    fn exp_part(&mut self) -> Result<(), String> {
        if matches!(self.peek(), Some('e' | 'E')) {
            self.pos += 1;
            if matches!(self.peek(), Some('+' | '-')) {
                self.pos += 1;
            }
            if !self.digits() {
                return Err("expected a digit in the exponent".to_string());
            }
        }
        Ok(())
    }
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

    #[test]
    fn preserves_number_text() {
        // Re-formatting is where two languages quietly disagree; an engine that does
        // not interpret a number should still hand it back unchanged.
        for n in ["1", "-0", "1.50", "1e100", "1E+2", "0.0", "1e-7", "-1.5e+300"] {
            assert_eq!(roundtrip(&format!("[{n}]")), format!("[{n}]"));
        }
    }

    /// Where preserving the token *diverges* from Python, deliberately.
    ///
    /// `CPython`'s round-trip is lossy for these: `json.loads("-0")` is the int `0`, and
    /// `1E+2` becomes the float `100.0`. Keeping the source text is stricter than matching
    /// Python here, and it is the behaviour an engine that never interprets a number needs —
    /// so it is written down rather than left to be rediscovered as a mismatch.
    #[test]
    fn the_token_survives_where_pythons_round_trip_would_not() {
        for (source, what_python_would_emit) in [("-0", "0"), ("1E+2", "100.0"), ("1.50", "1.5"), ("1e-7", "1e-07")] {
            assert_eq!(roundtrip(source), source, "{source} must survive verbatim");
            assert_ne!(source, what_python_would_emit, "case no longer illustrates the divergence");
        }
    }

    #[test]
    fn reads_escapes_and_surrogate_pairs() {
        assert_eq!(parse(r#""A😀\t""#).expect("parses"), Json::String("A\u{1F600}\t".to_string()));
        // Written as an explicit pair, the way Python emits it with ensure_ascii=True.
        assert_eq!(parse(r#""\ud83d\ude00""#).expect("parses"), Json::String("\u{1F600}".to_string()));
    }

    /// Every escape in the table, and the one that is read but never written.
    #[test]
    fn every_defined_escape_is_read() {
        for (written, means) in ESCAPES {
            let doc = format!("\"\\{written}\"");
            assert_eq!(parse(&doc).expect("parses"), Json::String(means.to_string()), "\\{written}");
        }
        assert!(parse(r#""\q""#).is_err(), "an undefined escape is an error");
    }

    /// A lone high surrogate is not an error — a malformed title must not make a whole
    /// tracker unreadable — and the position must not be left inside the failed attempt.
    #[test]
    fn a_lone_surrogate_becomes_the_replacement_char() {
        assert_eq!(parse(r#""\ud83d""#).expect("parses"), Json::String("\u{FFFD}".to_string()));
        // A high surrogate followed by a *non*-low escape: both survive, in order.
        assert_eq!(parse(r#""\ud83d\n""#).expect("parses"), Json::String("\u{FFFD}\n".to_string()));
        // And followed by an ordinary character, not an escape at all.
        assert_eq!(parse(r#""\ud83dx""#).expect("parses"), Json::String("\u{FFFD}x".to_string()));
    }

    #[test]
    fn a_truncated_or_non_hex_escape_is_an_error() {
        for bad in [r#""\u00"#, r#""\uZZZZ""#, r#""\u""#] {
            assert!(parse(bad).is_err(), "should reject {bad}");
        }
    }

    /// Each place a number can stop short. The dot and the `e` both commit to a digit, and
    /// a bare sign is not a number.
    #[test]
    fn a_number_that_stops_short_is_an_error() {
        for bad in ["1.", "1e", "1e+", "-", "[1.]", "[-]", "01", "-01"] {
            assert!(parse(bad).is_err(), "should reject {bad}");
        }
    }
}

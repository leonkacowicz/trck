//! The reader every parsing step shares: a position in the input, and the four primitives
//! that move it.
//!
//! Characters, not bytes. The escapes and the `\uXXXX` arithmetic are defined over code
//! points, and indexing a `Vec<char>` keeps a position meaningful in the terms the error
//! messages report it in.

use super::Json;

pub(super) struct Parser {
    pub(super) chars: Vec<char>,
    pub(super) pos: usize,
}

impl Parser {
    pub(super) fn new(text: &str) -> Parser {
        Parser { chars: text.chars().collect(), pos: 0 }
    }

    pub(super) fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    pub(super) fn bump(&mut self) -> Option<char> {
        let c = self.peek();
        if c.is_some() {
            self.pos += 1;
        }
        c
    }

    pub(super) fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(' ' | '\t' | '\n' | '\r')) {
            self.pos += 1;
        }
    }

    pub(super) fn expect(&mut self, want: char) -> Result<(), String> {
        match self.bump() {
            Some(c) if c == want => Ok(()),
            Some(c) => Err(format!("expected '{want}' at position {}, got '{c}'", self.pos - 1)),
            None => Err(format!("expected '{want}', got end of input")),
        }
    }

    /// A bare word — `true`, `false`, `null` — consumed one character at a time so a
    /// truncated one (`tru`) reports where it went wrong rather than which word was meant.
    pub(super) fn literal(&mut self, word: &str, value: Json) -> Result<Json, String> {
        for want in word.chars() {
            self.expect(want)?;
        }
        Ok(value)
    }
}

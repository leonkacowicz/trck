//! Reading an operation back out of the text [`super::Op::render`] wrote.
//!
//! Split from [`super`] because rendering and reading fail in different ways and are worth
//! reading separately: rendering cannot fail, and everything here can — this is the half that
//! meets a commit message written by an older engine, or by a hand.
//!
//! The grammar is exactly what `render` emits and nothing more: a verb, then operands, then
//! flags, with quoting as the only escape mechanism. Being lenient about anything else would
//! mean accepting records this engine cannot have produced, and quietly acting on them.

use super::Op;

/// Read back what [`super::Op::render`] wrote.
///
/// Operands come before flags because that is the order `render` emits them in; the first
/// bare `--name` ends the operands. A *quoted* token is never a flag however it starts, which
/// is what lets a leading `--` survive inside a title.
pub(super) fn parse(text: &str) -> Result<Op, String> {
    let tokens = tokenize(text)?;
    let mut it = tokens.into_iter();
    let Some(first) = it.next() else {
        return Err("empty operation".to_string());
    };
    let mut op = Op::new(&first.text);
    let mut rest: Vec<Token> = it.collect();
    rest.reverse(); // popped from the back, so this reads left to right
    while let Some(tok) = rest.pop() {
        match tok.flag_name() {
            None => op.operands.push(tok.text),
            Some(name) => {
                // The next token is this flag's value unless it is another flag, or there is
                // nothing left — which is how a valueless switch is told apart from one whose
                // value happens to follow.
                let value = match rest.last() {
                    Some(next) if next.flag_name().is_none() => rest.pop().map(|t| t.text),
                    _ => None,
                };
                op.flags.push((name.to_string(), value));
            },
        }
    }
    Ok(op)
}

/// One argument, and whether it arrived quoted.
///
/// The flag carries all the way through parsing: `"--title"` as a *value* must not be read as
/// the start of a flag, and only the quoting says which it was.
#[derive(Debug)]
struct Token {
    text: String,
    quoted: bool,
}

impl Token {
    /// The flag this token names, or `None` when it is an operand or a value.
    fn flag_name(&self) -> Option<&str> {
        (!self.quoted && self.text.starts_with("--")).then_some(self.text.as_str())
    }
}

/// Split a rendered op into its arguments, undoing the quoting.
fn tokenize(text: &str) -> Result<Vec<Token>, String> {
    let mut out = Vec::new();
    let mut chars = text.chars().peekable();
    while let Some(&c) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
            continue;
        }
        out.push(if c == '"' { quoted_token(&mut chars)? } else { bare_token(&mut chars) });
    }
    Ok(out)
}

/// A `"…"` run, with the escapes undone.
fn quoted_token(chars: &mut std::iter::Peekable<std::str::Chars>) -> Result<Token, String> {
    chars.next(); // the opening quote
    let mut text = String::new();
    loop {
        let Some(c) = chars.next() else {
            return Err(format!("unterminated quote in operation: \"{text}"));
        };
        match c {
            '"' => return Ok(Token { text, quoted: true }),
            // A trailing backslash cannot escape the closing quote away: falling through to
            // the unterminated error is right, since the text really is incomplete.
            '\\' => match chars.next() {
                Some('n') => text.push('\n'),
                Some('r') => text.push('\r'),
                Some('t') => text.push('\t'),
                // Anything else stands for itself — `\"` and `\\`, and leniently whatever a
                // later escape turns out to be, rather than refusing a record we can read.
                Some(other) => text.push(other),
                None => {},
            },
            _ => text.push(c),
        }
    }
}

/// A run up to the next whitespace.
fn bare_token(chars: &mut std::iter::Peekable<std::str::Chars>) -> Token {
    let mut text = String::new();
    while let Some(&c) = chars.peek() {
        if c.is_whitespace() {
            break;
        }
        text.push(c);
        chars.next();
    }
    Token { text, quoted: false }
}

#[cfg(test)]
mod tests {
    // Tests assert; that is their job. The crate denies unwrap/expect/panic because a
    // malformed tracker must produce a diagnostic rather than a stack trace, but a test
    // that cannot panic cannot fail.
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    /// Rendered, parsed, and equal to what went in. Every claim about a value surviving is
    /// this assertion with a different value.
    fn round_trips(op: &Op) {
        let text = op.render();
        let back = Op::parse(&text).unwrap_or_else(|e| panic!("parsing {text:?}: {e}"));
        assert_eq!(&back, op, "round trip changed the operation; rendered as {text:?}");
    }

    #[test]
    fn an_operation_survives_being_written_and_read() {
        round_trips(&Op::new("mv").operand("aaaaaaa").operand("done").flag("--resolution", Some("fixed")));
    }

    /// A switch must not acquire a value when read back, which is what makes it and a flag
    /// distinguishable at all — and a switch followed by a flag must not swallow its name.
    #[test]
    fn a_switch_stays_a_switch_even_before_a_flag() {
        round_trips(&Op::new("set").operand("aaaaaaa").switch("--auto", true));
        round_trips(&Op::new("set").operand("aaaaaaa").switch("--auto", true).flag("--priority", Some("high")));
    }

    /// A quoted token is never a flag, however it starts — which is what lets a title with a
    /// leading dash be an operand rather than an option.
    #[test]
    fn an_operand_beginning_with_a_dash_stays_an_operand() {
        round_trips(&Op::new("new").operand("--leading dashes").flag("--id", Some("aaaaaaa")));
        round_trips(&Op::new("set").operand("aaaaaaa").flag("--title", Some("--not-a-flag")));
    }

    /// The characters a title can carry that a naive record would lose.
    #[test]
    fn newlines_tabs_and_quotes_in_a_title_survive() {
        for title in ["two\nlines", "tab\there", "quote\"inside", "back\\slash", "'single'", "  padded  ", "-", "carriage\r\nreturn", ""] {
            round_trips(&Op::new("new").operand(title).flag("--id", Some("aaaaaaa")));
        }
    }

    /// Every verb the engine emits, through the round trip in the shape it actually produces.
    #[test]
    fn every_verbs_op_round_trips() {
        let ops = [
            Op::new("new")
                .operand("A title")
                .flag("--id", Some("aaaaaaa"))
                .flag("--slug", Some("a-title"))
                .flag("--priority", Some("high"))
                .flag("--points", Some("3"))
                .flag("--parent", Some("bbbbbbb"))
                .repeated("--requires", &["ccccccc"]),
            Op::new("mv").operand("aaaaaaa").operand("done").flag("--resolution", Some("wontfix")),
            Op::new("set").operand("aaaaaaa").switch("--auto", true).flag("--priority", Some("low")).repeated("--field", &["assignee=someone"]),
            Op::new("label").operand("aaaaaaa").repeated("--add", &["infra"]).repeated("--remove", &["urgent"]),
            Op::new("dep").operand("aaaaaaa").flag("--add", Some("bbbbbbb")).flag("--remove", Some("ccccccc")),
            Op::new("summary"),
            Op::new("normalize"),
        ];
        for op in &ops {
            round_trips(op);
        }
    }

    /// A malformed record is a diagnostic, not a panic — the crate's whole error posture, and
    /// here it is someone else's commit message being read.
    #[test]
    fn an_unterminated_quote_is_an_error() {
        let err = Op::parse(r#"set aaaaaaa --title "never closed"#).expect_err("unterminated");
        assert!(err.contains("unterminated quote"), "{err}");
    }

    #[test]
    fn an_empty_operation_is_an_error() {
        assert!(Op::parse("   ").is_err());
    }
}

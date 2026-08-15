//! What a verb was asked to do, and how it survives a round trip through a commit.
//!
//! An [`Op`] is the operation itself, structured rather than rendered: the verb, its
//! positional arguments, and the flags it was given. [`Op::render`] states it as the command
//! that would produce it again; [`parse`] reads that back.
//!
//! **The round trip is the point, not a convenience.** A pending commit has to be replayable
//! against a tree it was not built on, at any stacking depth, long after the process that
//! made it has gone. All that survives is the text in the commit message, so anything
//! `render` cannot express and `parse` cannot recover is data lost — which is why [`quote`]
//! is conservative and why every verb's op is tested through the round trip rather than
//! against an expected string.
//!
//! Values are the **resolved** ones, not what the user typed: an id prefix, a defaulted
//! priority and a generated id all mean something only against the tracker as it stood, so an
//! op recording them verbatim would replay into a different tracker as something else.

mod parse;

/// What the verb was asked to do. Flags carry `None` for switches that take no value.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Op {
    pub(crate) verb: String,
    pub(crate) operands: Vec<String>,
    pub(crate) flags: Vec<(String, Option<String>)>,
}

impl Op {
    pub(crate) fn new(verb: &str) -> Self {
        Self { verb: verb.to_string(), ..Self::default() }
    }

    /// Record a positional argument — the issue a verb acts on, and the status `mv` moves it
    /// to.
    pub(crate) fn operand(mut self, value: &str) -> Self {
        self.operands.push(value.to_string());
        self
    }

    /// Record `--name value`, skipping the flag entirely when the verb was not given it.
    /// Taking the `Option` here rather than at each call site is what keeps the verbs free of
    /// a conditional per flag.
    pub(crate) fn flag(mut self, name: &str, value: Option<&str>) -> Self {
        if let Some(v) = value {
            self.flags.push((name.to_string(), Some(v.to_string())));
        }
        self
    }

    /// Record `--name value` once per value — `--requires` and `--label` are repeatable.
    pub(crate) fn repeated<S: AsRef<str>>(mut self, name: &str, values: &[S]) -> Self {
        for v in values {
            self.flags.push((name.to_string(), Some(v.as_ref().to_string())));
        }
        self
    }

    /// Record a valueless switch when it was given.
    pub(crate) fn switch(mut self, name: &str, given: bool) -> Self {
        if given {
            self.flags.push((name.to_string(), None));
        }
        self
    }

    /// The op as the command line that would produce it again.
    pub(crate) fn render(&self) -> String {
        let mut out = self.verb.clone();
        for operand in &self.operands {
            out.push(' ');
            out.push_str(&quote(operand));
        }
        for (name, value) in &self.flags {
            out.push(' ');
            out.push_str(name);
            if let Some(v) = value {
                out.push(' ');
                out.push_str(&quote(v));
            }
        }
        out
    }

    /// Read back what [`Op::render`] wrote.
    pub(crate) fn parse(text: &str) -> Result<Op, String> {
        parse::parse(text)
    }

    /// The value of the first `--name` flag this op carries, when it has one.
    ///
    /// A query about an operation rather than a way to build one, which is why it lives
    /// beside the flags themselves: the subject line asks it, and so will replay.
    pub(crate) fn flag_value(&self, name: &str) -> Option<&str> {
        self.flags.iter().find(|(n, _)| n == name)?.1.as_deref()
    }
}

/// Quote a value whenever leaving it bare would change how it reads back.
///
/// Four ways that happens, and all four are reachable from a title someone typed: a space
/// splits one argument into two, a quote or backslash confuses the escape rules, a **leading
/// dash** turns a value into a flag, and a **newline ends the trailer**. The empty string has
/// to be quoted because nothing is not a token at all.
///
/// A line break is escaped rather than embedded, and that is not cosmetic: the rendered op
/// lives on one `Trck-Op:` line, and a literal newline would put the rest of the operation in
/// what git reads as the next paragraph — where nothing looks for it. Whatever this produces
/// must be a single line for every input.
fn quote(v: &str) -> String {
    if !v.is_empty() && !v.starts_with('-') && !v.contains([' ', '\t', '\n', '\r', '"', '\\', '\'']) {
        return v.to_string();
    }
    let mut out = String::with_capacity(v.len() + 2);
    out.push('"');
    for c in v.chars() {
        match c {
            '"' | '\\' => {
                out.push('\\');
                out.push(c);
            },
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    // Tests assert; that is their job. The crate denies unwrap/expect/panic because a
    // malformed tracker must produce a diagnostic rather than a stack trace, but a test
    // that cannot panic cannot fail.
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    /// The shape the issue names: the verb, the issue it acted on, and the flag that changed
    /// what it meant.
    #[test]
    fn an_op_renders_as_the_command_that_would_repeat_it() {
        let op = Op::new("mv").operand("aaaaaaa").operand("done").flag("--resolution", Some("fixed"));
        assert_eq!(op.render(), "mv aaaaaaa done --resolution fixed");
    }

    /// A flag the verb was not given leaves no trace: the record is what happened, not the
    /// full option surface.
    #[test]
    fn an_absent_flag_is_not_recorded() {
        let op = Op::new("mv").operand("aaaaaaa").operand("in-progress").flag("--resolution", None);
        assert_eq!(op.render(), "mv aaaaaaa in-progress");
    }

    /// `--auto` takes no value, and must not render a stray empty one.
    #[test]
    fn a_switch_renders_without_a_value() {
        let op = Op::new("set").operand("aaaaaaa").switch("--auto", true).switch("--force", false);
        assert_eq!(op.render(), "set aaaaaaa --auto");
    }

    /// A repeatable flag renders once per value rather than collapsing into a list.
    #[test]
    fn a_repeated_flag_renders_once_per_value() {
        let op = Op::new("new").operand("a title").repeated("--requires", &["aaaaaaa", "bbbbbbb"]);
        assert_eq!(op.render(), r#"new "a title" --requires aaaaaaa --requires bbbbbbb"#);
    }

    /// A verb with nothing to act on — `repo normalize` rewrites the whole index.
    #[test]
    fn an_op_without_operands_renders_the_verb_alone() {
        assert_eq!(Op::new("normalize").render(), "normalize");
    }

    /// The silent failure this quoting exists to prevent: a title is arbitrary text, and an
    /// unquoted one with a space would replay as two arguments.
    #[test]
    fn a_value_that_would_reparse_wrongly_is_quoted() {
        let op = Op::new("set").operand("aaaaaaa").flag("--title", Some("two words"));
        assert_eq!(op.render(), r#"set aaaaaaa --title "two words""#);
    }

    #[test]
    fn a_quote_and_a_backslash_in_a_value_are_escaped() {
        let op = Op::new("set").operand("aaaaaaa").flag("--title", Some(r#"a "quoted" c:\path"#));
        assert_eq!(op.render(), r#"set aaaaaaa --title "a \"quoted\" c:\\path""#);
    }

    /// An empty value has to survive the round trip too — bare, it would vanish.
    #[test]
    fn an_empty_value_is_quoted() {
        let op = Op::new("set").operand("aaaaaaa").flag("--spec", Some(""));
        assert_eq!(op.render(), r#"set aaaaaaa --spec """#);
    }

    /// A **leading dash** is the third way a value stops being a value: unquoted, it reads
    /// back as a flag, and the operation silently becomes a different one.
    #[test]
    fn a_value_beginning_with_a_dash_is_quoted() {
        let op = Op::new("set").operand("aaaaaaa").flag("--title", Some("--not-a-flag"));
        assert_eq!(op.render(), r#"set aaaaaaa --title "--not-a-flag""#);
    }

    /// **One line, always.** The rendering lives on a single `Trck-Op:` line, so a literal
    /// newline in it would put the rest of the operation in what git reads as the next
    /// paragraph — where nothing looks for it, and the record is silently half lost.
    #[test]
    fn a_rendered_op_is_always_a_single_line() {
        for title in ["two\nlines", "carriage\rreturn", "crlf\r\nboth", "trailing\n"] {
            let text = Op::new("new").operand(title).render();
            assert_eq!(text.lines().count(), 1, "rendered across lines: {text:?}");
            assert!(!text.contains(['\n', '\r']), "{text:?}");
        }
    }
}

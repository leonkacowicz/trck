//! What a mutating verb produces instead of writing files.
//!
//! A verb derives a whole new tracker state and then has to put it somewhere. Splitting the
//! two — [`Changeset`] for the bytes, [`Op`] for the intent — is what lets a second
//! destination exist: a directory applies the changeset with `write`/`rename`/`remove`, and a
//! commit-building backend (`#sqzr7nk`) turns the same edits into blobs and a tree, with the
//! `Op` as the commit's replayable record of what was asked for.
//!
//! Paths here are **tracker-relative** (`index.jsonl`, `items/aaaaaaa-a-title.md`). An
//! absolute path is a fact about one backend; a tracker that lives in a git ref has a tree to
//! address, not a directory.

use crate::issue::Issue;
use std::path::PathBuf;

/// One file's worth of change.
///
/// `Rename` is its own variant rather than a delete plus a write because the two are not the
/// same thing to the destination: git records a rename as a rename, and `set --slug` moving a
/// body must not read as an unrelated file appearing and another vanishing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Edit {
    Write { path: PathBuf, contents: String },
    Rename { from: PathBuf, to: PathBuf },
    Delete { path: PathBuf },
}

/// Everything a verb changed, in the order it must be applied.
///
/// `rows` is the derived index the edits already encode — carried alongside because the
/// caller validates against rows, not against rendered text it would have to re-parse.
#[derive(Debug, Default)]
pub(crate) struct Changeset {
    pub(crate) rows: Vec<Issue>,
    pub(crate) edits: Vec<Edit>,
}

impl Changeset {
    pub(crate) fn new(rows: Vec<Issue>, edits: Vec<Edit>) -> Self {
        Self { rows, edits }
    }
}

/// What the verb was asked to do, structured rather than rendered.
///
/// The point is replay: a backend that records an operation has to be able to state it as the
/// command that would produce it again, and a string assembled at the call site cannot be
/// checked. Flags carry `None` for the switches that take no value (`--auto`).
///
/// Values are the **resolved** ones, not what the user typed: an id prefix, a defaulted
/// priority and a generated id all mean something only against the tracker as it stood, so an
/// op recording them verbatim would replay into a different tracker as something else.
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
}

/// Quote a value only when leaving it bare would change how it parses.
///
/// A title is arbitrary text and lands in this record verbatim, so an unquoted one with a
/// space in it would replay as two arguments — the failure is silent, which is why the rule
/// is here and tested rather than left to whoever writes the trailer.
fn quote(v: &str) -> String {
    if !v.is_empty() && !v.contains([' ', '\t', '\n', '"', '\\', '\'']) {
        return v.to_string();
    }
    let mut out = String::with_capacity(v.len() + 2);
    out.push('"');
    for c in v.chars() {
        if c == '"' || c == '\\' {
            out.push('\\');
        }
        out.push(c);
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
}

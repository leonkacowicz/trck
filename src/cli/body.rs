//! Where `new` gets an issue's prose.
//!
//! Git's model rather than three inventions of our own: `--body` is `-m`, `--body-file` is
//! `-F` and takes `-` for stdin, and `--empty` says the title is the whole issue. One flag
//! at most — they are different answers to the same question, and silently preferring one
//! would file an issue nobody asked for.
//!
//! **Absent all three, what happens depends on whether anyone is there to type.** On a
//! terminal it is a human who can be handed a template; with no terminal it is a script,
//! and a script that meant to write a body and forgot must be told, not guessed at. That
//! asymmetry is the whole point of the change: `trck new "title"` in CI used to file an
//! empty template and exit 0.

use super::Args;

/// Which flag said where the body comes from.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum BodySpec {
    /// `--body TEXT`
    Text(String),
    /// `--body-file PATH`
    File(String),
    /// `--body-file -`
    Stdin,
    /// `--empty`
    Empty,
    /// None of them.
    Unsaid,
}

/// The flags, named once, for the error that lists them.
const FLAGS: &str = "--body, --body-file or --empty";

/// Which body source the arguments name, or the refusal for naming more than one.
pub(super) fn body_spec(args: &Args) -> Result<BodySpec, String> {
    // Collected rather than ranked, so "more than one" is a length and not a precedence
    // rule nobody wrote down.
    let mut said: Vec<BodySpec> = Vec::new();
    if let Some(t) = args.opt("--body") {
        said.push(BodySpec::Text(t.to_string()));
    }
    if let Some(p) = args.opt("--body-file") {
        said.push(if p == "-" { BodySpec::Stdin } else { BodySpec::File(p.to_string()) });
    }
    if args.has("--empty") {
        said.push(BodySpec::Empty);
    }
    match said.len() {
        0 => Ok(BodySpec::Unsaid),
        1 => Ok(said.remove(0)),
        _ => Err(format!("new: {FLAGS} are different ways to give a body; pick one")),
    }
}

/// The body text itself.
///
/// `interactive` is whether stdin is a terminal, passed in rather than read here so the
/// rule is testable without one. It only matters for [`BodySpec::Unsaid`]: everything else
/// means the caller already said what it wanted.
pub(super) fn resolve(spec: &BodySpec, title: &str, interactive: bool) -> Result<String, String> {
    let text = match spec {
        BodySpec::Text(t) => t.clone(),
        BodySpec::File(p) => std::fs::read_to_string(p).map_err(|e| format!("new: {p}: {e}"))?,
        BodySpec::Stdin => read_stdin()?,
        // The template's first line and nothing else. Derived from the template rather than
        // written out again, so the heading a title-only issue gets is the heading every
        // other issue gets.
        BodySpec::Empty => heading(title),
        BodySpec::Unsaid => unsaid(title, interactive)?,
    };
    Ok(terminated(&text))
}

/// What "no flag at all" means, which depends entirely on who is running the command.
///
/// A terminal means a human, who gets the template to fill in — that is what `#nabxbdk`
/// replaces with an editor, and until then it is exactly today's behaviour. No terminal
/// means a script, and a script that meant to write a body and forgot must be told.
fn unsaid(title: &str, interactive: bool) -> Result<String, String> {
    if interactive {
        return super::editor::edit(title);
    }
    Err(format!("new: nobody is at a terminal, so there is nothing to fill in a body; pass {FLAGS}"))
}

fn read_stdin() -> Result<String, String> {
    let mut buf = String::new();
    std::io::Read::read_to_string(&mut std::io::stdin(), &mut buf).map_err(|e| format!("new: stdin: {e}"))?;
    Ok(buf)
}

/// The template's first line, with the title in it.
fn heading(title: &str) -> String {
    let first = crate::verbs::TEMPLATE.lines().next().unwrap_or("# {title}");
    format!("{}\n", first.replace("{title}", title))
}

/// Exactly one trailing newline, because a body is a file and files end in one.
///
/// An empty body stays empty rather than becoming a lone newline: `--body ""` is a
/// deliberate nothing, and writing a blank line for it would be inventing content.
fn terminated(text: &str) -> String {
    let trimmed = text.trim_end_matches('\n');
    if trimmed.is_empty() { String::new() } else { format!("{trimmed}\n") }
}

#[cfg(test)]
mod tests {
    // Tests assert; that is their job. The crate denies unwrap/expect/panic because a
    // malformed tracker must produce a diagnostic rather than a stack trace, but a test
    // that cannot panic cannot fail.
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    fn args(argv: &[&str]) -> Args {
        super::super::parse_args(&argv.iter().map(|s| (*s).to_string()).collect::<Vec<_>>())
    }

    #[test]
    fn each_flag_names_its_own_source() {
        assert_eq!(body_spec(&args(&["new", "T", "--body", "hi"])).expect("spec"), BodySpec::Text("hi".into()));
        assert_eq!(body_spec(&args(&["new", "T", "--body-file", "b.md"])).expect("spec"), BodySpec::File("b.md".into()));
        assert_eq!(body_spec(&args(&["new", "T", "--body-file", "-"])).expect("spec"), BodySpec::Stdin);
        assert_eq!(body_spec(&args(&["new", "T", "--empty"])).expect("spec"), BodySpec::Empty);
        assert_eq!(body_spec(&args(&["new", "T"])).expect("spec"), BodySpec::Unsaid);
    }

    /// Two answers to one question. Preferring either silently files an issue whose prose
    /// is not the prose the caller passed.
    #[test]
    fn two_body_flags_are_refused_rather_than_ranked() {
        for argv in [
            vec!["new", "T", "--body", "hi", "--body-file", "b.md"],
            vec!["new", "T", "--body", "hi", "--empty"],
            vec!["new", "T", "--body-file", "b.md", "--empty"],
        ] {
            let err = body_spec(&args(&argv)).expect_err("refused");
            assert!(err.contains("pick one"), "{err}");
        }
    }

    #[test]
    fn inline_text_is_the_body() {
        assert_eq!(resolve(&BodySpec::Text("hello".into()), "T", false).expect("body"), "hello\n");
    }

    /// The same text through either flag has to produce the same file, or `--body` and
    /// `--body-file` are two features rather than two spellings.
    #[test]
    fn a_trailing_newline_is_normalised_not_doubled() {
        let once = resolve(&BodySpec::Text("hello\n".into()), "T", false).expect("body");
        let twice = resolve(&BodySpec::Text("hello\n\n\n".into()), "T", false).expect("body");
        assert_eq!(once, "hello\n");
        assert_eq!(twice, "hello\n");
    }

    #[test]
    fn an_empty_body_stays_empty_rather_than_becoming_a_blank_line() {
        assert_eq!(resolve(&BodySpec::Text(String::new()), "T", false).expect("body"), "");
    }

    #[test]
    fn empty_is_the_heading_and_nothing_else() {
        let body = resolve(&BodySpec::Empty, "A Title", false).expect("body");
        assert_eq!(body, "# A Title\n");
        assert!(!body.contains("## Summary"), "the template leaked into --empty: {body}");
    }

    /// The row that makes `trck new` safe to script: no body, no terminal, no issue.
    #[test]
    fn no_flag_and_no_terminal_names_every_flag() {
        let err = resolve(&BodySpec::Unsaid, "T", false).expect_err("refused");
        for flag in ["--body", "--body-file", "--empty"] {
            assert!(err.contains(flag), "the refusal must name {flag}: {err}");
        }
    }

    #[test]
    fn a_body_file_that_is_not_there_names_it() {
        let err = resolve(&BodySpec::File("/nonexistent/nope.md".into()), "T", false).expect_err("refused");
        assert!(err.contains("nope.md"), "{err}");
    }
}

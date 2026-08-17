//! Per-verb help.
//!
//! `trck <verb> --help` has to answer about the verb, not the program. It used to print the
//! same summary whatever came before it, which is the least useful moment to be told what
//! trck is in general.
//!
//! The text is inherited rather than invented: it comes from the argparse definitions the
//! engine's predecessor carried, where every option already had a sentence written for it.
//! Rewriting them from scratch would have quietly changed what the tool claims to do.
//!
//! What keeps it honest is the test at the bottom: every flag a verb accepts must be
//! documented here, and every flag documented here must be one the verb accepts. Help that
//! drifts from the parser is worse than no help, because it is believed.

/// One verb's help: what it is for, how to call it, and what each part means.
struct VerbHelp {
    verb: &'static str,
    /// One line, as the verb list would describe it.
    tagline: &'static str,
    usage: &'static str,
    /// A paragraph, wrapped when rendered rather than pre-wrapped here.
    blurb: &'static str,
    /// Positional arguments — or, for `repo`, its subcommands.
    args: &'static [(&'static str, &'static str)],
    opts: &'static [(&'static str, &'static str)],
    /// Empty when there is nothing worth showing.
    example: &'static str,
    /// Set when this verb is another one under a second name. Its options are that verb's,
    /// so they are documented once, there, and pointed at from here — repeating fifteen
    /// filter flags in two places is how the two copies start disagreeing.
    alias_of: &'static str,
}

/// Accepted by every verb and explained once, in the global help, rather than repeated two
/// dozen times in a list of options that are actually about the verb.
const GLOBAL_OPTS: &[(&str, &str)] = &[
    ("--dir DIR", "the tracker to act on, overriding discovery and $TRCK_DIR"),
    ("--ref REF", "read the tracker from this git ref, overriding discovery and $TRCK_REF"),
];

mod edit;
mod maintain;
mod read;

/// Every verb's help, in the order the groups are declared.
///
/// Three slices rather than one table: the text is ~300 lines of prose and it is the
/// part that changes most, so it lives beside the dispatch grouping it mirrors.
const GROUPS: &[&[VerbHelp]] = &[edit::VERBS, read::VERBS, maintain::VERBS];

fn entries() -> impl Iterator<Item = &'static VerbHelp> {
    GROUPS.iter().copied().flatten()
}

/// Wrap `text` to `width` columns, prefixing every line with `indent`.
///
/// Written out rather than pre-wrapped in the table because the table is edited far more
/// often than this is, and a paragraph stored as one string is one a person can rewrite
/// without re-flowing it by hand.
fn wrap(text: &str, width: usize, indent: &str) -> String {
    let mut lines: Vec<String> = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        if !line.is_empty() && line.chars().count() + 1 + word.chars().count() > width {
            lines.push(std::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    if !line.is_empty() {
        lines.push(line);
    }
    let mut out = String::new();
    for l in &lines {
        out.push_str(indent);
        out.push_str(l);
        out.push('\n');
    }
    out
}

/// A two-column block: the term, then its description wrapped in the remaining width.
fn columns(rows: &[(&str, &str)], width: usize) -> String {
    let gutter = rows.iter().map(|(t, _)| t.chars().count()).max().unwrap_or(0) + 2;
    let mut out = String::new();
    for (term, desc) in rows {
        let indent = " ".repeat(gutter + 2);
        let body = wrap(desc, width.saturating_sub(gutter + 2), &indent);
        out.push_str("  ");
        out.push_str(term);
        out.push_str(&" ".repeat(gutter.saturating_sub(term.chars().count())));
        out.push_str(body.trim_start());
    }
    out
}

/// Help for one verb, or `None` when it has none — in which case the caller falls back to
/// the program's own help rather than saying nothing.
pub(crate) fn for_verb(verb: &str) -> Option<String> {
    let h = entries().find(|h| h.verb == verb)?;
    let width = 88;
    let mut out = format!("trck {} — {}\n\n", h.verb, h.tagline);
    out.push_str("usage: ");
    out.push_str(h.usage);
    out.push_str("\n\n");
    out.push_str(&wrap(h.blurb, width, ""));
    if !h.args.is_empty() {
        let label = if h.verb == "repo" { "\nsubcommands:\n" } else { "\narguments:\n" };
        out.push_str(label);
        out.push_str(&columns(h.args, width));
    }
    if !h.opts.is_empty() {
        out.push_str("\noptions:\n");
        out.push_str(&columns(h.opts, width));
    }
    if !h.alias_of.is_empty() {
        out.push_str("\noptions: every option `");
        out.push_str(h.alias_of);
        out.push_str("` takes — see `trck ");
        out.push_str(h.alias_of);
        out.push_str(" --help`.\n");
    }
    out.push_str("\nglobal:\n");
    out.push_str(&columns(GLOBAL_OPTS, width));
    if !h.example.is_empty() {
        out.push_str("\nexample:\n");
        for line in h.example.lines() {
            out.push_str("  ");
            out.push_str(line.trim());
            out.push('\n');
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    // Tests assert; that is their job. The crate denies unwrap/expect/panic because a
    // malformed tracker must produce a diagnostic rather than a stack trace, but a test
    // that cannot panic cannot fail.
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    /// The verb list and the help table are two lists of the same thing, and a verb that
    /// gains help nobody can reach is as useless as one with none.
    #[test]
    fn every_verb_the_binary_offers_has_help() {
        for verb in crate::cli::tables::VERBS {
            assert!(for_verb(verb).is_some(), "`{verb}` has no help");
        }
    }

    /// Help that disagrees with the parser is worse than absent help, because it is
    /// believed. Both directions: nothing documented that would be refused, nothing
    /// accepted that goes unmentioned.
    #[test]
    fn documented_options_are_exactly_the_accepted_ones() {
        for (verb, _, accepted) in crate::cli::tables::KNOWN_FLAGS {
            let Some(h) = entries().find(|h| h.verb == *verb) else {
                continue;
            };
            let documented: Vec<&str> = h.opts.iter().map(|(o, _)| o.split_whitespace().next().unwrap_or(o)).collect();
            for flag in &documented {
                assert!(accepted.contains(flag), "`{verb}` documents {flag}, which it would refuse");
            }
            // An alias documents nothing of its own; its options live with the verb it
            // stands for, and the rendering says where.
            if !h.alias_of.is_empty() {
                continue;
            }
            for flag in *accepted {
                // Explained once in the global section instead of two dozen times.
                if crate::cli::tables::GLOBAL_FLAGS.contains(flag) {
                    continue;
                }
                assert!(documented.contains(flag), "`{verb}` accepts {flag} and does not document it");
            }
        }
    }

    #[test]
    fn the_rendering_carries_usage_blurb_and_every_option() {
        let text = for_verb("new").expect("new has help");
        assert!(text.contains("usage: trck new <title>"), "{text}");
        assert!(text.contains("--priority"), "{text}");
        assert!(text.contains("--requires"), "{text}");
        assert!(text.contains("example:"), "{text}");
        assert!(text.contains("--dir"), "global options missing: {text}");
    }

    /// The one place a line can run away is a long option description, since the term
    /// column is as wide as the widest flag.
    #[test]
    fn nothing_renders_wider_than_it_should() {
        for verb in crate::cli::tables::VERBS {
            let text = for_verb(verb).expect("help");
            for line in text.lines() {
                assert!(line.chars().count() <= 100, "`{verb}` renders a {}-column line: {line}", line.chars().count());
            }
        }
    }

    #[test]
    fn an_unknown_verb_has_no_help_rather_than_empty_help() {
        assert!(for_verb("nonesuch").is_none());
    }
}

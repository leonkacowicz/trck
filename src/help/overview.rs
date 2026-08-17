//! The program's own help — `trck --help`, and the fallback when a verb has none.

use super::render::columns_at;
use super::{GLOBAL_OPTS, GROUPS, entries};

/// What to type, and every verb with one line on what it does.
///
/// Someone who runs `trck --help` is looking for the verb they want, so that is all this
/// is — a map of the vocabulary, grouped, pointing at the per-verb help for the detail.
/// It used to open with three lines on what trck is and how it is specified, which is what
/// a reader has already decided by the time they have the binary installed; that belongs to
/// the README, where they came from.
pub(crate) fn overview(version: &str) -> String {
    let width = 88;
    // Measured across every group, and over the global options too, so the one column the
    // whole page is read down does not move.
    let gutter = entries().map(|h| h.verb.chars().count()).chain(GLOBAL_OPTS.iter().map(|(o, _)| o.chars().count())).max().unwrap_or(0) + 2;
    let mut out = format!("trck {version} — in-repo issue tracker\n\nusage: trck <verb> [args] [options]\n");
    for (label, verbs) in GROUPS {
        let rows: Vec<(&str, &str)> = verbs.iter().map(|h| (h.verb, h.tagline)).collect();
        out.push('\n');
        out.push_str(label);
        out.push_str(":\n");
        out.push_str(&columns_at(&rows, width, gutter));
    }
    out.push_str("\nglobal:\n");
    out.push_str(&columns_at(GLOBAL_OPTS, width, gutter));
    out.push_str("\n`trck <verb> --help` for one verb's arguments, options and examples.\n");
    out
}

#[cfg(test)]
mod tests {
    // Tests assert; that is their job. The crate denies unwrap/expect/panic because a
    // malformed tracker must produce a diagnostic rather than a stack trace, but a test
    // that cannot panic cannot fail.
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    /// The overview is a map of the verbs, and `for_verb`'s contract one level up:
    /// everything you can type is listed, with the same one-liner the verb's own help opens
    /// with. What trck *is* belongs to the README — whoever reads this has the binary.
    #[test]
    fn it_names_every_verb_and_what_it_does() {
        let text = overview("9.9.9");
        assert!(text.starts_with("trck 9.9.9 — in-repo issue tracker\n"), "{text}");
        assert!(text.contains("usage: trck <verb>"), "{text}");
        for verb in crate::cli::tables::VERBS {
            let h = entries().find(|h| h.verb == *verb).expect("help");
            assert!(text.contains(h.tagline), "`{verb}` is not described in the overview: {text}");
        }
        for (opt, _) in GLOBAL_OPTS {
            assert!(text.contains(opt), "the overview drops {opt}: {text}");
        }
        assert!(text.contains("trck <verb> --help"), "nothing points at per-verb help: {text}");
        for line in text.lines() {
            assert!(line.chars().count() <= 100, "the overview renders a {}-column line: {line}", line.chars().count());
        }
    }

    /// One table broken by headings, not three tables: a term column measured per group
    /// would step in and out down the page as each group's longest verb differs.
    #[test]
    fn it_aligns_every_group_on_one_column() {
        let text = overview("9.9.9");
        let described: Vec<&str> = text.lines().filter(|l| l.starts_with("  ")).collect();
        // Where the description begins: just past the last run of padding. No description
        // contains a double space, so the last one in the line is always the gutter.
        let at = |l: &str| l.rfind("  ").map(|i| i + 2);
        assert!(described.len() >= crate::cli::tables::VERBS.len(), "{text}");
        for line in &described {
            assert_eq!(at(line), at(described[0]), "`{line}` starts its description elsewhere");
        }
    }
}

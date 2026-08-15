//! Turning an operation into the one line a reader of `git log --oneline` sees.
//!
//! Split from [`super`] because the subject and the trailer have opposite jobs. A subject is
//! prose for a human: one line, readable, and free to drop what will not fit. A trailer is
//! data: lossless, and never rewritten for looks. Keeping them apart is what stops the second
//! rule quietly acquiring the first one's exceptions.
//!
//! Everything a subject drops is still recoverable from the trailer beneath it. Nothing is
//! ever recovered from a subject.

use super::super::super::op::Op;

/// How much of a title a subject carries before it is cut short.
///
/// Git's own convention is roughly 50 characters for a subject, and every tool that renders a
/// log assumes something like it. The prefix (`new #aaaaaaa: `) is part of the budget, so this
/// is the whole line rather than the title alone.
const SUBJECT_WIDTH: usize = 72;

/// The subject line for an operation.
pub(super) fn subject(op: &Op) -> String {
    let body = match op.verb.as_str() {
        "new" => format!("new {}{}", hash(op.flag_value("--id")), op.operands.first().map_or(String::new(), |t| format!(": {t}"))),
        "mv" => moved(op),
        "set" => format!("set {} {}", hash(op.operands.first().map(String::as_str)), settings(op)),
        "label" => format!("label {} {}", hash(op.operands.first().map(String::as_str)), edges(op, "", "")),
        "dep" => format!("dep {} {}", hash(op.operands.first().map(String::as_str)), edges(op, "#", "#")),
        // Everything else: the verb, and the issue it acted on when it named one. A verb that
        // acts on the whole tracker (`summary`, `normalize`) has no operand and reads as
        // itself, and one that acts on an issue (`edit`) says which — so a verb added later
        // gets a subject that is at least right, rather than one that silently drops the id.
        other => format!("{other} {}", hash(op.operands.first().map(String::as_str))),
    };
    fit(&body, SUBJECT_WIDTH)
}

/// `done #aaaaaaa (wontfix)` — the destination status leads, because that is what a reader of
/// the log is scanning for, and the resolution qualifies it.
fn moved(op: &Op) -> String {
    let (id, status) = (op.operands.first().map(String::as_str), op.operands.get(1).map_or("mv", String::as_str));
    match op.flag_value("--resolution") {
        Some(res) => format!("{status} {} ({res})", hash(id)),
        None => format!("{status} {}", hash(id)),
    }
}

/// `priority=high spec=none --auto` — the edits `set` was asked for, without their dashes,
/// because in a subject the dashes are noise and the `key=value` is the information.
fn settings(op: &Op) -> String {
    let parts: Vec<String> = op
        .flags
        .iter()
        .map(|(name, value)| {
            let key = name.trim_start_matches('-');
            value.as_ref().map_or_else(|| format!("--{key}"), |v| format!("{key}={v}"))
        })
        .collect();
    parts.join(" ")
}

/// `+infra -urgent`, or `+#bbbbbbb` for a dependency — one sign per edge, so a reader sees the
/// direction without reading the flag names.
fn edges(op: &Op, add_prefix: &str, remove_prefix: &str) -> String {
    let parts: Vec<String> = op
        .flags
        .iter()
        .filter_map(|(name, value)| {
            let v = value.as_ref()?;
            match name.as_str() {
                "--add" => Some(format!("+{add_prefix}{v}")),
                "--remove" => Some(format!("-{remove_prefix}{v}")),
                _ => None,
            }
        })
        .collect();
    parts.join(" ")
}

/// `#aaaaaaa`, or nothing when the op names no issue — an id is what makes a log line
/// greppable, and a bare id reads as a word.
fn hash(id: Option<&str>) -> String {
    id.map_or_else(String::new, |id| format!("#{id}"))
}

/// Fit text onto a subject line: single-spaced, one line, no longer than `width`.
///
/// The two halves are one operation because a subject is never wanted without both. Collapsing
/// is not cosmetic — a title can hold a newline and a subject cannot, so git would read
/// everything past the first line as the body and the trailer would end up inside what looks
/// like prose. Truncation is marked, so a cut title does not read as the whole one.
///
/// Counted in `char`s rather than bytes: slicing a multi-byte character in half panics, and a
/// title is arbitrary text. Nothing is lost either way — the trailer below carries it exactly.
fn fit(s: &str, width: usize) -> String {
    let line = s.split_whitespace().collect::<Vec<&str>>().join(" ");
    if line.chars().count() <= width {
        return line;
    }
    let keep: String = line.chars().take(width.saturating_sub(1)).collect();
    format!("{}…", keep.trim_end())
}

#[cfg(test)]
mod tests {
    // Tests assert; that is their job. The crate denies unwrap/expect/panic because a
    // malformed tracker must produce a diagnostic rather than a stack trace, but a test
    // that cannot panic cannot fail.
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    /// The four shapes the issue documents.
    #[test]
    fn each_verb_has_its_documented_subject() {
        let new = Op::new("new").operand("A title").flag("--id", Some("aaaaaaa"));
        assert_eq!(subject(&new), "new #aaaaaaa: A title");

        let done = Op::new("mv").operand("aaaaaaa").operand("done").flag("--resolution", Some("fixed"));
        assert_eq!(subject(&done), "done #aaaaaaa (fixed)");

        let set = Op::new("set").operand("aaaaaaa").flag("--priority", Some("high"));
        assert_eq!(subject(&set), "set #aaaaaaa priority=high");

        let dep = Op::new("dep").operand("aaaaaaa").flag("--add", Some("bbbbbbb"));
        assert_eq!(subject(&dep), "dep #aaaaaaa +#bbbbbbb");
    }

    /// A move with no resolution says where it went and stops.
    #[test]
    fn a_move_without_a_resolution_names_the_status_alone() {
        assert_eq!(subject(&Op::new("mv").operand("aaaaaaa").operand("in-progress")), "in-progress #aaaaaaa");
    }

    #[test]
    fn a_label_edit_shows_the_direction_of_each_edge() {
        let op = Op::new("label").operand("aaaaaaa").repeated("--add", &["infra"]).repeated("--remove", &["urgent"]);
        assert_eq!(subject(&op), "label #aaaaaaa +infra -urgent");
    }

    /// A valueless switch keeps its dashes: `auto=` would be a lie about its shape.
    #[test]
    fn a_switch_survives_into_the_subject_as_a_switch() {
        assert_eq!(subject(&Op::new("set").operand("aaaaaaa").switch("--auto", true)), "set #aaaaaaa --auto");
    }

    /// A verb that acts on the whole tracker names itself and nothing else.
    #[test]
    fn a_whole_tracker_verb_is_its_own_subject() {
        assert_eq!(subject(&Op::new("normalize")), "normalize");
        assert_eq!(subject(&Op::new("summary")), "summary");
    }

    /// A verb with no arm of its own still says which issue it touched. `edit` arrived after
    /// this module did, and a fallback that dropped the id would have been silently wrong.
    #[test]
    fn a_verb_without_its_own_shape_still_names_the_issue() {
        assert_eq!(subject(&Op::new("edit").operand("aaaaaaa")), "edit #aaaaaaa");
    }

    /// One line whatever the title was, because git reads everything past the first line as
    /// the body — a newline there would put the trailer inside prose.
    #[test]
    fn a_title_with_newlines_still_makes_one_line() {
        let op = Op::new("new").operand("two\nlines\tand\ttabs").flag("--id", Some("aaaaaaa"));
        assert_eq!(subject(&op), "new #aaaaaaa: two lines and tabs");
    }

    /// Truncation is marked, so a cut title does not read as the whole one.
    #[test]
    fn a_long_subject_is_cut_and_says_so() {
        let op = Op::new("new").operand(&"x".repeat(200)).flag("--id", Some("aaaaaaa"));
        let line = subject(&op);
        assert_eq!(line.chars().count(), SUBJECT_WIDTH, "{line}");
        assert!(line.ends_with('\u{2026}'), "{line}");
    }

    /// Counted in characters, not bytes: slicing a multi-byte character in half panics, and a
    /// title is arbitrary text.
    #[test]
    fn truncating_does_not_split_a_multibyte_character() {
        let op = Op::new("new").operand(&"é".repeat(200)).flag("--id", Some("aaaaaaa"));
        assert_eq!(subject(&op).chars().count(), SUBJECT_WIDTH);
    }
}

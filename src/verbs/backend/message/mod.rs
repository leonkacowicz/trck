//! The commit a tracker write leaves behind.
//!
//! `git log --oneline trck-issues` is the tracker's changelog, so the subject is generated
//! and structured rather than free text: `new #aaaaaaa: A title`, `done #aaaaaaa (wontfix)`,
//! `set #aaaaaaa priority=high`, `dep #aaaaaaa +#bbbbbbb`. Building it is [`subject`].
//!
//! **The trailer is the load-bearing half.** `Trck-Op:` records the operation itself, so a
//! pending commit can be replayed against a tree it was not built on — at any stacking depth,
//! long after the verb that produced it has left memory. It doubles as the audit log.
//!
//! The two halves have different jobs and so different rules. A subject may collapse and
//! truncate; a trailer must be lossless and must fit on one line. Anything the subject drops
//! is still recoverable from the trailer beneath it.

use super::super::op::Op;
use subject::subject;

mod subject;

/// The key the operation is recorded under. Git's own trailer convention — `Key: value` in the
/// last paragraph — so `git log --format=%(trailers:key=Trck-Op)` reads it without help.
const TRAILER_KEY: &str = "Trck-Op:";

/// The whole commit message: a structured subject, a blank line, and the operation.
pub(super) fn message(op: &Op) -> String {
    format!("{}\n\n{TRAILER_KEY} {}\n", subject(op), op.render())
}

/// The operation a commit message records, or `None` when it carries none.
///
/// `None` rather than an error for a commit with no trailer: the tracker branch can hold
/// commits this engine did not write — a seeding commit, a hand-made fix — and those are not
/// malformed, they are simply not operations. A trailer that *is* there and does not parse is
/// an error, because that is a record which was meant to be read and cannot be.
///
/// The **last** trailer wins. A rebase or a squash can stack messages, and the operation this
/// commit performed is the one written last.
pub(crate) fn op_of(message: &str) -> Result<Option<Op>, String> {
    let Some(line) = message.lines().rev().find_map(|l| l.trim().strip_prefix(TRAILER_KEY)) else {
        return Ok(None);
    };
    Op::parse(line.trim()).map(Some)
}

#[cfg(test)]
mod tests {
    // Tests assert; that is their job. The crate denies unwrap/expect/panic because a
    // malformed tracker must produce a diagnostic rather than a stack trace, but a test
    // that cannot panic cannot fail.
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    /// What the whole thing is for: the operation comes back out of the message it was
    /// written into, unchanged — for every verb, and for a title carrying everything that
    /// would break a naive record.
    #[test]
    fn the_operation_round_trips_through_the_message() {
        let ops = [
            Op::new("new").operand("A title\nwith a newline\tand a tab").flag("--id", Some("aaaaaaa")).flag("--priority", Some("high")),
            Op::new("mv").operand("aaaaaaa").operand("done").flag("--resolution", Some("wontfix")),
            Op::new("set").operand("aaaaaaa").switch("--auto", true).flag("--title", Some("--leading dash")),
            Op::new("label").operand("aaaaaaa").repeated("--add", &["infra"]).repeated("--remove", &["urgent"]),
            Op::new("dep").operand("aaaaaaa").flag("--remove", Some("bbbbbbb")),
            Op::new("summary"),
            Op::new("normalize"),
        ];
        for op in &ops {
            let text = message(op);
            let back = op_of(&text).expect("parses").expect("a trailer is present");
            assert_eq!(&back, op, "message was:\n{text}");
        }
    }

    /// Three lines, always: subject, blank, trailer. A title with a newline in it would
    /// otherwise put the trailer inside what git reads as the body.
    #[test]
    fn a_message_is_a_subject_a_blank_line_and_a_trailer() {
        let op = Op::new("new").operand("two\nlines").flag("--id", Some("aaaaaaa"));
        let text = message(&op);
        assert_eq!(text.lines().count(), 3, "{text:?}");
        assert_eq!(text.lines().nth(1), Some(""), "{text:?}");
        assert!(text.lines().nth(2).is_some_and(|l| l.starts_with(TRAILER_KEY)), "{text:?}");
    }

    /// A commit the engine did not write is not malformed — the tracker branch can hold a
    /// seeding commit or a hand-made fix, and those simply are not operations.
    #[test]
    fn a_message_without_a_trailer_carries_no_operation() {
        assert_eq!(op_of("seed the tracker\n\nnothing structured here\n").expect("no error"), None);
    }

    /// A trailer that *is* there and does not parse is an error: it was meant to be read.
    /// A diagnostic, never a panic — this is someone else's commit message.
    #[test]
    fn an_unparseable_trailer_is_an_error_not_a_panic() {
        let err = op_of("subject\n\nTrck-Op: set aaaaaaa --title \"never closed\n").expect_err("malformed");
        assert!(err.contains("unterminated quote"), "{err}");
    }

    /// The last one wins: a squash can stack messages, and the operation this commit
    /// performed is the one written last.
    #[test]
    fn the_last_trailer_is_the_commits_own() {
        let msg = "subject\n\nTrck-Op: normalize\n\nsquashed in\n\nTrck-Op: mv aaaaaaa done\n";
        let op = op_of(msg).expect("parses").expect("present");
        assert_eq!(op, Op::new("mv").operand("aaaaaaa").operand("done"));
    }
}

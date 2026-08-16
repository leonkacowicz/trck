//! Reading and writing `index.jsonl`.
//!
//! One JSON object per line, rows sorted by id, a trailing newline when non-empty.
//! Failures name the file and the line: an index is the tracker's source of truth, so a
//! row that cannot be understood stops everything rather than being skipped.

use crate::issue::Issue;
use crate::json::parse;
use std::collections::{BTreeMap, BTreeSet};
#[cfg(test)]
use std::path::Path;

/// Parse index text into rows, naming `origin` in any failure.
///
/// Split from reading the file so a source that is not the working tree's index — a
/// file handed to `diff`, a revision's blob, stdin — parses through the same contract
/// and reports against its own name.
pub(crate) fn parse_index(text: &str, origin: &str) -> Result<Vec<Issue>, String> {
    let mut rows = Vec::new();
    for (n, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let n = n + 1;
        let raw = parse(line).map_err(|e| format!("{origin} line {n}: invalid JSON ({e})"))?;
        let row = Issue::from_json(&raw).map_err(|e| format!("{origin} line {n}: {e}"))?;
        rows.push(row);
    }
    check_unique_ids(&rows, origin)?;
    Ok(rows)
}

/// Refuse an index that keys two rows to one id.
///
/// A duplicate is a structural defect, not a recoverable inconsistency: it makes the
/// in-memory model ambiguous, so it fails here rather than as a check the mutating verbs
/// would write straight past. Every duplicate is collected before failing — fixing them
/// one round-trip at a time is miserable.
fn check_unique_ids(rows: &[Issue], origin: &str) -> Result<(), String> {
    // Statuses ride along with the count: a duplicate almost always arrives from a bad
    // merge, and which two statuses collided is what tells you which side to keep.
    let mut seen: BTreeMap<&str, (usize, BTreeSet<&str>)> = BTreeMap::new();
    for r in rows {
        let e = seen.entry(r.id.as_str()).or_default();
        e.0 += 1;
        e.1.insert(r.status.as_str());
    }
    let dupes: Vec<String> = seen
        .into_iter()
        .filter(|&(_, (n, _))| n > 1)
        .map(|(id, (n, statuses))| format!("  #{id} appears {n} times (statuses: {})", statuses.into_iter().collect::<Vec<_>>().join(", ")))
        .collect();
    if dupes.is_empty() {
        return Ok(());
    }
    Err(format!("{origin}: ids must be unique, but {} id(s) are repeated:\n{}", dupes.len(), dupes.join("\n")))
}

/// Serialise rows to index text: sorted by id, canonical form, trailing newline when
/// non-empty. Empty in, empty out — not a lone newline.
pub(crate) fn render_index(rows: &[Issue]) -> String {
    let mut sorted: Vec<&Issue> = rows.iter().collect();
    sorted.sort_by(|a, b| a.id.cmp(&b.id));
    let mut out = String::new();
    for r in sorted {
        out.push_str(&r.to_canonical().to_json());
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    // Tests assert; that is their job. The crate denies unwrap/expect/panic because a
    // malformed tracker must produce a diagnostic rather than a stack trace, but a test
    // that cannot panic cannot fail.
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    const A: &str = r#"{"id": "bbbbbbb", "slug": "b", "title": "B", "status": "backlog", "priority": "high"}"#;
    const B: &str = r#"{"id": "aaaaaaa", "slug": "a", "title": "A", "status": "backlog", "priority": "low"}"#;

    #[test]
    fn duplicate_ids_are_reported_with_their_statuses() {
        // A duplicate usually arrives from a bad merge, so the statuses of the colliding
        // rows are the useful part: they say which two versions of the row survived.
        let text = format!("{A}\n{}\n", r#"{"id": "bbbbbbb", "slug": "b", "title": "B", "status": "done", "priority": "high"}"#);
        let err = parse_index(&text, "index.jsonl").expect_err("duplicate must fail");
        assert_eq!(
            err,
            "index.jsonl: ids must be unique, but 1 id(s) are repeated:\n  \
             #bbbbbbb appears 2 times (statuses: backlog, done)"
        );
    }

    #[test]
    fn round_trips_byte_for_byte() {
        let text = format!("{B}\n{A}\n");
        let rows = parse_index(&text, "index.jsonl").expect("parses");
        assert_eq!(render_index(&rows), text);
    }

    #[test]
    fn rows_are_written_sorted_by_id() {
        let rows = parse_index(&format!("{A}\n{B}\n"), "index.jsonl").expect("parses");
        assert_eq!(render_index(&rows), format!("{B}\n{A}\n"));
    }

    #[test]
    fn blank_lines_are_skipped() {
        let rows = parse_index(&format!("\n{A}\n\n  \n"), "index.jsonl").expect("parses");
        assert_eq!(rows.len(), 1);
    }

    #[test]
    fn an_empty_index_renders_as_nothing() {
        assert_eq!(render_index(&[]), "");
        assert!(parse_index("", "index.jsonl").expect("parses").is_empty());
    }

    #[test]
    fn a_bad_line_names_the_file_and_line() {
        let err = parse_index(&format!("{A}\nnot json\n"), "index.jsonl").expect_err("rejects");
        assert!(err.starts_with("index.jsonl line 2: invalid JSON"), "{err}");
    }

    #[test]
    fn a_wrongly_typed_row_names_the_file_line_and_field() {
        let bad = r#"{"id": "ccccccc", "slug": "c", "title": "C", "status": "backlog", "priority": "high", "points": "lots"}"#;
        let err = parse_index(&format!("{A}\n{bad}\n"), "somewhere.jsonl").expect_err("rejects");
        assert_eq!(err, "somewhere.jsonl line 2: field 'points' must be an integer, got 'lots'");
    }

    /// Round-trip an index built to break this, and require the bytes back.
    ///
    /// The tests above cover the shapes someone thought to write down; this one covers the
    /// shapes that break a serialiser — every escape the encoder owes, characters beyond the
    /// BMP, every optional field at once in canonical order, and the defaults that must come
    /// out omitted. See [`crate::test_index`] for what each row is for, and for why it is a
    /// deliberate fixture rather than this repository's own tracker.
    #[test]
    fn a_hostile_index_round_trips_byte_for_byte() {
        let text = crate::test_index::HOSTILE_INDEX;
        let rows = parse_index(text, "hostile.jsonl").unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(rows.len(), 7, "the fixture lost a row");
        assert_eq!(render_index(&rows), text, "the hostile index did not round-trip");
    }

    /// The bundled example is still what the engine would write — both files.
    ///
    /// A *data* check rather than an engine one, which is why the two generated files are
    /// asserted together in one place instead of one each beside the function that produces
    /// them: the question is whether the committed example has gone stale, and half an answer
    /// to that is no answer. The README screenshots are generated from it.
    ///
    /// It skips when absent, because a consumer of this crate has no reason to have it — and
    /// unlike the hostile fixture, nothing here depends on it being present to mean anything.
    #[test]
    fn the_bundled_example_is_canonical() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples").join("action-game");
        let (Ok(text), Ok(summary)) = (std::fs::read_to_string(dir.join("index.jsonl")), std::fs::read_to_string(dir.join("SUMMARY.md"))) else {
            return;
        };
        let rows = parse_index(&text, "the example").unwrap_or_else(|e| panic!("{e}"));
        assert!(!rows.is_empty(), "the example parsed to nothing");
        assert_eq!(render_index(&rows), text, "the example's index.jsonl did not round-trip");
        let got = crate::summary::generate_summary(&crate::graph::Graph::new(rows));
        assert_eq!(got, summary, "the example's SUMMARY.md is stale");
    }

    #[test]
    fn duplicate_ids_are_refused_all_at_once() {
        let text = format!("{A}\n{A}\n{B}\n{B}\n");
        let err = parse_index(&text, "index.jsonl").expect_err("rejects");
        assert!(err.contains("#aaaaaaa appears 2 times"), "{err}");
        assert!(err.contains("#bbbbbbb appears 2 times"), "{err}");
    }
}

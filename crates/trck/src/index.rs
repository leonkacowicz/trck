//! Reading and writing `index.jsonl`.
//!
//! One JSON object per line, rows sorted by id, a trailing newline when non-empty.
//! Failures name the file and the line: an index is the tracker's source of truth, so a
//! row that cannot be understood stops everything rather than being skipped.

use crate::issue::Issue;
use crate::json::parse;
use std::collections::BTreeMap;
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
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for r in rows {
        *counts.entry(r.id.as_str()).or_default() += 1;
    }
    let dupes: Vec<String> = counts
        .into_iter()
        .filter(|&(_, n)| n > 1)
        .map(|(id, n)| format!("  #{id} appears {n} times"))
        .collect();
    if dupes.is_empty() {
        return Ok(());
    }
    Err(format!("{origin}: duplicate ids\n{}", dupes.join("\n")))
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

    const A: &str =
        r#"{"id": "bbbbbbb", "slug": "b", "title": "B", "status": "backlog", "priority": "high"}"#;
    const B: &str =
        r#"{"id": "aaaaaaa", "slug": "a", "title": "A", "status": "backlog", "priority": "low"}"#;

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
        assert_eq!(
            err,
            "somewhere.jsonl line 2: field 'points' must be an integer, got 'lots'"
        );
    }

    /// Round-trip every index committed in this repo and require the bytes back.
    ///
    /// The unit tests above cover the shapes someone thought to write down. This covers
    /// the ones nobody did: 230-odd real rows carrying real titles, labels, links and
    /// timestamps. Canonical serialisation has to be byte-identical to the Python
    /// engine's, and these files *are* its output — they are written by `repo
    /// normalize` and committed.
    #[test]
    fn real_indexes_round_trip_byte_for_byte() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("crates/trck is two levels below the repo root")
            .to_path_buf();
        let mut checked = 0;
        for rel in ["issues/index.jsonl", "examples/action-game/index.jsonl"] {
            let path = root.join(rel);
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue; // a consumer of this crate need not have the tracker
            };
            let rows = parse_index(&text, rel).unwrap_or_else(|e| panic!("{rel}: {e}"));
            assert!(!rows.is_empty(), "{rel} parsed to nothing");
            assert_eq!(render_index(&rows), text, "{rel} did not round-trip");
            checked += 1;
        }
        assert!(checked > 0, "no committed index found to check");
    }

    #[test]
    fn duplicate_ids_are_refused_all_at_once() {
        let text = format!("{A}\n{A}\n{B}\n{B}\n");
        let err = parse_index(&text, "index.jsonl").expect_err("rejects");
        assert!(err.contains("#aaaaaaa appears 2 times"), "{err}");
        assert!(err.contains("#bbbbbbb appears 2 times"), "{err}");
    }
}

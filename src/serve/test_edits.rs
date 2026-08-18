//! What each staged edit means, as operations.
//!
//! Its own file, the way `test_http.rs` is: `edits.rs` is the mapping and this is the table of
//! what it maps, and the two are read for different reasons. Everything here goes through the
//! module's own entry points, so the file states the contract rather than the internals.

// Tests assert; that is their job. The crate denies unwrap/expect/panic because a
// malformed tracker must produce a diagnostic rather than a stack trace, but a test
// that cannot panic cannot fail.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::edits::{ops_for, parse_request};
use crate::issue::Issue;
use crate::json::Json;
use crate::verbs::Op;

fn row() -> Issue {
    let json =
        r#"{"id": "aaaaaaa", "slug": "alpha", "title": "Alpha", "status": "backlog", "priority": "medium", "labels": ["ui"], "depends_on": ["bbbbbbb"]}"#;
    crate::index::parse_index(json, "index.jsonl").expect("parses").pop().expect("one row")
}

fn rendered(field: &str, value: &Json) -> Vec<String> {
    ops_for(&row(), field, value).expect("mapped").iter().map(Op::render).collect()
}

fn s(v: &str) -> Json {
    Json::String(v.to_string())
}

fn arr(vs: &[&str]) -> Json {
    Json::Array(vs.iter().map(|v| s(v)).collect())
}

/// The mapping the pending panel promises. These strings are the commands `commandFor`
/// renders, minus the `trck` prefix — `tests/app_js.rs` asserts that from the other end,
/// by running the page's own function under node.
#[test]
fn a_scalar_field_maps_to_the_command_the_panel_shows() {
    assert_eq!(rendered("status", &s("done")), ["mv aaaaaaa done"]);
    assert_eq!(rendered("priority", &s("high")), ["set aaaaaaa --priority high"]);
    assert_eq!(rendered("parent", &s("ccccccc")), ["set aaaaaaa --parent ccccccc"]);
}

/// `points` is a number to the page and an integer to the tracker. Its source text is
/// what crosses, because re-formatting a number is where two languages disagree.
#[test]
fn a_numeric_field_keeps_the_text_it_arrived_as() {
    assert_eq!(rendered("points", &Json::Number("3".into())), ["set aaaaaaa --points 3"]);
    assert_eq!(rendered("points", &s("3")), ["set aaaaaaa --points 3"]);
}

/// A list field carries the whole desired list; the ops are the difference. That is how a
/// control that edits a list works, and it means the page never computes a delta against a
/// row it might have stale.
#[test]
fn a_list_field_becomes_the_ops_that_make_it_true() {
    assert_eq!(rendered("labels", &arr(&["ui", "perf"])), ["label aaaaaaa --add perf"]);
    assert_eq!(rendered("labels", &arr(&["perf"])), ["label aaaaaaa --add perf --remove ui"]);
    assert_eq!(rendered("requires", &arr(&["bbbbbbb", "ccccccc"])), ["dep aaaaaaa --add ccccccc"]);
}

/// A removal comes first, so replacing one edge with another never has both in the graph
/// at once — which is the difference between a legal swap and a refusal for a cycle that
/// was only ever going to exist between two commits.
#[test]
fn a_replaced_dependency_is_removed_before_the_new_one_is_added() {
    assert_eq!(rendered("requires", &arr(&["ccccccc"])), ["dep aaaaaaa --remove bbbbbbb", "dep aaaaaaa --add ccccccc"]);
}

/// An edit that asks for what is already there is not an operation. Committing one would
/// be a commit whose diff is empty, on a branch whose history is meant to be readable.
#[test]
fn an_edit_that_changes_nothing_produces_no_op() {
    assert!(ops_for(&row(), "labels", &arr(&["ui"])).expect("mapped").is_empty());
    assert!(ops_for(&row(), "requires", &arr(&["bbbbbbb"])).expect("mapped").is_empty());
}

/// The vocabulary is closed. A field this endpoint does not offer is refused by name
/// rather than guessed at — the request never names a verb, so there is nothing a client
/// can ask for that was not intended.
#[test]
fn an_unknown_field_is_refused_by_name() {
    let err = ops_for(&row(), "resolution", &s("fixed")).expect_err("refused");
    assert!(err.contains("resolution"), "{err}");
    assert!(err.contains("not an editable field"), "{err}");
}

#[test]
fn a_value_of_the_wrong_shape_is_refused() {
    assert!(ops_for(&row(), "status", &arr(&["done"])).is_err(), "a list is not a status");
    assert!(ops_for(&row(), "labels", &s("ui")).is_err(), "a string is not a list of labels");
    assert!(ops_for(&row(), "labels", &Json::Array(vec![Json::Bool(true)])).is_err(), "a label is a string");
}

#[test]
fn the_request_document_is_the_pages_own_vocabulary() {
    let edits = parse_request(r#"{"edits": [{"id": "aaaaaaa", "field": "status", "value": "done"}]}"#).expect("parsed");
    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0].id, "aaaaaaa");
    assert_eq!(edits[0].field, "status");
    assert_eq!(edits[0].value, Json::String("done".into()));
}

#[test]
fn a_request_of_the_wrong_shape_says_what_was_expected() {
    for bad in ["", "[]", "{}", r#"{"edits": {}}"#, r#"{"edits": [{"id": "a"}]}"#, r#"{"edits": [{"field": "status", "value": "done"}]}"#] {
        let err = parse_request(bad).expect_err(bad);
        assert!(!err.is_empty(), "{bad} was refused without saying why");
    }
}

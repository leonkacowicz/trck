//! Applying a batch, against a tracker on disk.
//!
//! Through [`super::apply::batch`] rather than by building an `Outcome` by hand, which is what
//! these tests used to do: a hand-built one asserts the shape of a document and nothing about
//! whether the path that fills it in agrees. A directory tracker is enough — the verbs do not
//! know which kind of tracker they are writing to, and what the ref backend adds on top (a
//! commit, a push, a rebuild after somebody else's) needs a remote, which is `tests/serve_edits.rs`.

// Tests assert; that is their job. The crate denies unwrap/expect/panic because a malformed
// tracker must produce a diagnostic rather than a stack trace, but a test that cannot panic
// cannot fail.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::apply::batch;
use crate::discovery::tests::Tmp;
use crate::discovery::{Ctx, ITEMS_DIR, Source};

/// A tracker holding two issues, one of them already carrying a label.
fn tracker(tag: &str) -> (Tmp, Ctx) {
    let tmp = Tmp::new(tag);
    let dir = tmp.tracker("issues");
    std::fs::create_dir_all(dir.join(ITEMS_DIR)).expect("mkdir");
    let rows = [("aaaaaaa", "alpha", "Alpha", "[\"ui\"]"), ("bbbbbbb", "beta", "Beta", "[]")];
    let mut index = Vec::new();
    for (id, slug, title, labels) in rows {
        std::fs::write(dir.join(ITEMS_DIR).join(format!("{id}-{slug}.md")), format!("# {title}\n")).expect("body");
        index.push(format!(
            "{{\"id\": \"{id}\", \"slug\": \"{slug}\", \"title\": \"{title}\", \"status\": \"backlog\", \"priority\": \"medium\", \"labels\": {labels}}}"
        ));
    }
    std::fs::write(dir.join("index.jsonl"), index.join("\n") + "\n").expect("index");
    let ctx = Ctx::load(Source::Dir(dir), false).expect("loads");
    (tmp, ctx)
}

fn edits(body: &str) -> String {
    format!("{{\"edits\": [{body}]}}")
}

/// The page reads `ok` to decide whether to reload and `error` to show what the engine said.
/// A directory tracker has no ref, so `sha` is null — the page falls back to reloading, which
/// is what it would have done anyway.
#[test]
fn a_batch_that_lands_reports_what_each_operation_said() {
    let (_tmp, ctx) = tracker("apply-ok");
    let out = batch(&ctx, &edits(r#"{"id": "aaaaaaa", "field": "priority", "value": "high"}"#)).expect("a batch");
    assert!(out.ok(), "{}", out.json());
    let json = out.json();
    assert!(json.contains("\"ok\": true"), "{json}");
    assert!(json.contains("\"error\": null"), "{json}");
    assert!(json.contains("\"sha\": null"), "a directory tracker has no ref: {json}");
    // The verb's own words, not a rewording of them.
    assert!(json.contains("aaaaaaa"), "{json}");
    // And the tracker actually moved.
    let rows = crate::verbs::load_rows(&ctx).expect("rows");
    assert_eq!(rows.iter().find(|r| r.id == "aaaaaaa").expect("the row").priority, "high");
}

/// A refusal carries the engine's own diagnostic and leaves the tracker alone.
#[test]
fn a_refused_edit_changes_nothing_and_says_what_the_engine_said() {
    let (_tmp, ctx) = tracker("apply-refuse");
    let out = batch(&ctx, &edits(r#"{"id": "aaaaaaa", "field": "priority", "value": "nosuch"}"#)).expect("a batch");
    assert!(!out.ok(), "an unknown priority must be refused");
    let json = out.json();
    assert!(json.contains("bad priority 'nosuch'"), "{json}");
    assert!(json.contains("\"applied\": []"), "nothing landed, so nothing should be reported as landed: {json}");
    let rows = crate::verbs::load_rows(&ctx).expect("rows");
    assert_eq!(rows.iter().find(|r| r.id == "aaaaaaa").expect("the row").priority, "medium", "a refused edit changed the tracker");
}

/// A batch stops at the first refusal, and what already landed stays landed — each operation
/// is its own commit. "It failed" and "nothing happened" are different sentences, and the
/// answer has to be able to say the first without implying the second.
#[test]
fn a_batch_stops_at_the_first_refusal_and_still_reports_what_landed() {
    let (_tmp, ctx) = tracker("apply-batch");
    let body = edits(
        r#"{"id": "aaaaaaa", "field": "status", "value": "in-progress"},
           {"id": "aaaaaaa", "field": "priority", "value": "nosuch"},
           {"id": "bbbbbbb", "field": "status", "value": "in-progress"}"#,
    );
    let out = batch(&ctx, &body).expect("a batch");
    assert!(!out.ok());
    assert!(out.json().contains("bad priority"), "{}", out.json());

    let rows = crate::verbs::load_rows(&ctx).expect("rows");
    let status = |id: &str| rows.iter().find(|r| r.id == id).expect("the row").status.clone();
    assert_eq!(status("aaaaaaa"), "in-progress", "the first edit did not land");
    assert_eq!(status("bbbbbbb"), "backlog", "the batch kept going past a refusal");
}

/// A list field carries the whole desired list and becomes the difference — including the case
/// where there is no difference, which must not produce a commit whose diff is empty.
#[test]
fn a_list_field_applies_only_what_changed() {
    let (_tmp, ctx) = tracker("apply-list");
    let out = batch(&ctx, &edits(r#"{"id": "aaaaaaa", "field": "labels", "value": ["ui", "perf"]}"#)).expect("a batch");
    assert!(out.ok(), "{}", out.json());
    let rows = crate::verbs::load_rows(&ctx).expect("rows");
    assert_eq!(rows.iter().find(|r| r.id == "aaaaaaa").expect("the row").labels, ["perf", "ui"]);

    let out = batch(&ctx, &edits(r#"{"id": "aaaaaaa", "field": "labels", "value": ["perf", "ui"]}"#)).expect("a batch");
    assert!(out.ok(), "{}", out.json());
    assert!(out.json().contains("\"applied\": []"), "an edit that changes nothing is not an operation: {}", out.json());
}

/// An id is resolved the way every verb resolves one, so a prefix works here too — and an
/// ambiguous or absent one is refused rather than guessed at.
#[test]
fn an_issue_is_named_the_way_every_other_verb_names_one() {
    let (_tmp, ctx) = tracker("apply-id");
    let out = batch(&ctx, &edits(r#"{"id": "aaa", "field": "status", "value": "done"}"#)).expect("a batch");
    assert!(out.ok(), "a unique prefix should resolve: {}", out.json());

    let out = batch(&ctx, &edits(r#"{"id": "zzzzzzz", "field": "status", "value": "done"}"#)).expect("a batch");
    assert!(!out.ok());
    assert!(out.json().contains("zzzzzzz"), "the refusal must name what could not be found: {}", out.json());
}

/// A request that is not a request is an `Err` rather than a refused batch: the caller answers
/// the first with a 400 about the request and the second with a 422 about the tracker, and a
/// page that could not tell them apart would retry the one that can never succeed.
#[test]
fn a_malformed_request_is_a_different_kind_of_failure() {
    let (_tmp, ctx) = tracker("apply-malformed");
    for bad in ["", "not json", "{}", r#"{"edits": [{"id": "aaaaaaa"}]}"#] {
        assert!(batch(&ctx, bad).is_err(), "{bad:?} is not a batch");
    }
    // Well formed, and refused by the tracker: an outcome, not an error.
    let out = batch(&ctx, &edits(r#"{"id": "aaaaaaa", "field": "resolution", "value": "fixed"}"#)).expect("a batch");
    assert!(!out.ok());
    assert!(out.json().contains("not an editable field"), "{}", out.json());
}

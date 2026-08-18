//! The one part of the suite that is not fixtures: the page's JavaScript.
//!
//! The app script is a compiled-in asset, so nothing in the Rust suite would otherwise
//! execute it — a syntax error or a broken pure function would ship silently and only
//! surface in a browser. These tests lift named declarations out of the asset by text
//! and run them under `node`, which is the same trick `tests/test_html.py` used before
//! the tool was folded in.
//!
//! Lifting by text rather than parsing JavaScript is deliberate: every top-level
//! function in the script closes with a `}` in column zero, so the block boundary is
//! unambiguous, and a parser would be a second implementation of the thing under test.
//!
//! Skipped when `node` is absent, so a contributor without it is not blocked.

// An integration test asserts; that is its job. The crate denies unwrap/expect/panic
// because a malformed tracker must produce a diagnostic rather than a stack trace, but a
// test that cannot panic cannot fail.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::process::Command;

const APP_JS: &str = include_str!("../assets/app.js");

fn have_node() -> bool {
    Command::new("node").arg("--version").output().is_ok_and(|o| o.status.success())
}

/// Lift top-level `const` and `function` declarations out of the script by name.
///
/// A declaration ends either at the end of its own line (a plain constant or a one-line
/// arrow helper) or at the first `}` or `};` in column zero — every top-level block in
/// the script closes that way, which is what makes the boundary unambiguous without
/// parsing JavaScript.
fn lift(names: &[&str]) -> String {
    let lines: Vec<&str> = APP_JS.lines().collect();
    let mut out = Vec::new();
    for name in names {
        let opener =
            |l: &str| l.starts_with(&format!("const {name} ")) || l.starts_with(&format!("const {name}=")) || l.starts_with(&format!("function {name}("));
        let start = lines.iter().position(|l| opener(l)).unwrap_or_else(|| panic!("could not lift `{name}` out of the app script"));
        // Balanced on the first line? Then it is a one-liner and stands alone.
        let one_line = lines[start].ends_with(';') || lines[start].ends_with(',');
        if one_line {
            out.push(lines[start].to_string());
            continue;
        }
        let mut block = vec![lines[start]];
        for next in &lines[start + 1..] {
            block.push(next);
            if *next == "}" || *next == "};" {
                break;
            }
        }
        out.push(block.join("\n"));
    }
    out.join("\n")
}

/// Run a snippet under node, returning its stdout.
fn run_node(script: &str) -> String {
    // A unique directory per call: these tests run in parallel, and sharing a path
    // means one snippet silently executes another's code.
    use std::sync::atomic::{AtomicUsize, Ordering};
    static N: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!("trck-js-{}-{}", std::process::id(), N.fetch_add(1, Ordering::Relaxed)));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join("snippet.mjs");
    std::fs::write(&path, script).expect("write snippet");
    let out = Command::new("node").arg(&path).output().expect("run node");
    let _ = std::fs::remove_dir_all(&dir);
    assert!(out.status.success(), "node failed: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn the_script_parses() {
    if !have_node() {
        return;
    }
    // A syntax error in an embedded asset is invisible to the Rust compiler: it is a
    // string. This is the only thing that would catch one before a browser does.
    let dir = std::env::temp_dir().join(format!("trck-js-check-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join("app.js");
    std::fs::write(&path, APP_JS).expect("write");
    let out = Command::new("node").args(["--check"]).arg(&path).output().expect("run node");
    let _ = std::fs::remove_dir_all(&dir);
    assert!(out.status.success(), "app.js does not parse: {}", String::from_utf8_lossy(&out.stderr));
}

#[test]
fn the_isotonic_fit_keeps_rows_ordered_and_apart() {
    if !have_node() {
        return;
    }
    // The graph view's row placement: substituting `y = x - offset` turns "keep the
    // order, stay a gap apart" into "y is non-decreasing", so the closest legal
    // placement is the isotonic fit. Cheap to get subtly wrong, invisible on screen.
    let script = format!("{}\nconst out = isotonic([5, 1, 3]);\nconsole.log(JSON.stringify(out));\n", lift(&["isotonic"]));
    let out = run_node(&script);
    let vals: Vec<f64> = out.trim().trim_matches(|c| c == '[' || c == ']').split(',').filter_map(|v| v.trim().parse().ok()).collect();
    assert_eq!(vals.len(), 3, "{out}");
    assert!(vals.windows(2).all(|w| w[0] <= w[1]), "the fit must be non-decreasing: {vals:?}");
}

#[test]
fn the_board_gives_ready_a_column_and_takes_those_cards_out_of_backlog() {
    if !have_node() {
        return;
    }
    // The board's invariant is that every card sits in exactly one column. `ready` is
    // not a status, so it can only have a column if the cards it shows are subtracted
    // from the status column they would otherwise be in — and the two counts have to add
    // back up to the status total, or the board is quietly lying about how much is left.
    //
    // Position is asserted too, not only membership: `ready` sits immediately after the
    // column it takes from, so the row reads as the path a card travels.
    let script = format!(
        "{}\n\
         const statuses = [{{name: 'backlog'}}, {{name: 'in-progress'}}, {{name: 'done'}}];\n\
         const shown = [\n\
           {{id: 'a', status: 'backlog', ready: true}},\n\
           {{id: 'b', status: 'backlog', ready: false}},\n\
           {{id: 'c', status: 'backlog', ready: false}},\n\
           {{id: 'd', status: 'in-progress', ready: false}},\n\
         ];\n\
         const cols = boardColumns(statuses, shown);\n\
         console.log(JSON.stringify(cols.map(c => [c.name, c.items.map(i => i.id)])));\n",
        lift(&["boardColumns"])
    );
    let out = run_node(&script);
    let want = r#"[["backlog",["b","c"]],["ready",["a"]],["in-progress",["d"]],["done",[]]]"#;
    assert_eq!(out.trim(), want, "{out}");
}

#[test]
fn a_view_only_applies_the_facets_it_declares() {
    if !have_node() {
        return;
    }
    // `ready` answers "what now?" with one ranked list; slicing it by status would hide
    // the answer. So a view declares which facets apply and the rest are ignored — and
    // a value the facet offers no box for is exempt rather than filtered away.
    let script = format!(
        "{}\n\
         const vocab = new Set(['backlog', 'done']);\n\
         const sel = new Set(['backlog']);\n\
         console.log(JSON.stringify([\n\
           passesFacet('list', 'status', sel, 'backlog', vocab),\n\
           passesFacet('list', 'status', sel, 'done', vocab),\n\
           passesFacet('list', 'status', sel, 'foreign', vocab),\n\
           passesFacet('ready', 'status', sel, 'done', vocab),\n\
         ]));\n",
        lift(&["VIEW_FACETS", "facetsFor", "passesFacet"])
    );
    let out = run_node(&script);
    assert!(out.contains("true,false,true,true"), "{out}");
}

#[test]
fn the_graph_opens_with_done_work_omitted() {
    if !have_node() {
        return;
    }
    // The graph tab's opening state is what the reader is asked to make sense of before
    // they have found a control, so it starts on the live graph: done issues out. The
    // done-*chains* filter stays off underneath it — that one is about whole settled
    // components and only becomes visible again once someone unticks `omit done`.
    let script = format!("{}\nconsole.log(JSON.stringify(GRAPH_DEFAULTS));\n", lift(&["GRAPH_DEFAULTS"]));
    let out = run_node(&script);
    assert_eq!(out.trim(), r#"{"includeDone":false,"omitDone":true}"#, "{out}");

    // And the defaults have to be the ones the view actually starts from, or the
    // constant above is a comment that happens to compile.
    assert!(APP_JS.contains("graphIncludeDone: GRAPH_DEFAULTS.includeDone"), "state must seed graphIncludeDone from GRAPH_DEFAULTS");
    assert!(APP_JS.contains("graphOmitDone: GRAPH_DEFAULTS.omitDone"), "state must seed graphOmitDone from GRAPH_DEFAULTS");
}

#[test]
fn selecting_an_issue_leaves_the_view_pane_where_it_was() {
    if !have_node() {
        return;
    }
    // Selecting re-renders the active view, and every renderer empties its container to
    // rebuild it — which is also the scrolling element, so the offsets go to zero and the
    // graph the reader had scrolled to jumps out from under the pointer. Selection changes
    // only which node is accented, never the layout, so the offsets are still the right
    // ones afterwards: hold them across the rebuild.
    let script = format!(
        "{}\n\
         let renders = 0;\n\
         // A stand-in for the pane: rendering wipes it, which is what zeroes the offsets.\n\
         const box = {{ scrollTop: 420, scrollLeft: 130 }};\n\
         keepingScroll(box, () => {{ renders++; box.scrollTop = 0; box.scrollLeft = 0; }});\n\
         // A view with no pane element must still render rather than throw.\n\
         keepingScroll(null, () => {{ renders++; }});\n\
         console.log(JSON.stringify([box.scrollTop, box.scrollLeft, renders]));\n",
        lift(&["keepingScroll"])
    );
    let out = run_node(&script);
    assert_eq!(out.trim(), "[420,130,2]", "{out}");

    // And the helper has to be what selection actually goes through, or it is dead code
    // that passes its own test. The pane ids match the view names one-for-one.
    assert!(APP_JS.contains("keepingScroll($('#' + state.view), renderActiveView)"), "select() must re-render the active view through keepingScroll");
}

/// **The two ends of one promise.** The pending panel shows the command a staged edit would
/// run; `POST /edits` runs that edit in this process. If the two ever disagree, the panel is
/// lying about what the Apply button does — which is the one thing a page that both *shows*
/// and *applies* an edit must not do.
///
/// The commands below are the contract, and both ends assert against them:
/// `src/serve/edits.rs::a_scalar_field_maps_to_the_command_the_panel_shows` asserts the engine
/// builds exactly these (minus the `trck` prefix, which an op does not render), and this
/// asserts the page's own `commandFor` renders them. Change one and the other fails.
#[test]
fn the_panel_renders_the_command_the_server_will_run() {
    if !have_node() {
        return;
    }
    let script = format!(
        "const DATA = {{ config: {{ cmd: 'trck' }} }};\n\
         {}\n\
         console.log(JSON.stringify([\n\
           commandFor('aaaaaaa', 'status', 'done'),\n\
           commandFor('aaaaaaa', 'priority', 'high'),\n\
         ]));\n",
        lift(&["commandFor"])
    );
    let out = run_node(&script);
    assert_eq!(out.trim(), r#"["trck mv aaaaaaa done","trck set aaaaaaa --priority high"]"#, "{out}");
}

/// A staged edit crosses the wire as `{id, field, value}` — the same three things the panel
/// splits its key into. Asserted because that split is string arithmetic on `::`, and the day
/// a field name contains one it would quietly post a different edit than the one on screen.
#[test]
fn a_staged_edit_becomes_the_document_the_server_takes() {
    if !have_node() {
        return;
    }
    let script = format!(
        "const state = {{ edits: {{ 'aaaaaaa::status': 'done', 'bbbbbbb::priority': 'low' }} }};\n\
         {}\n\
         console.log(JSON.stringify(stagedEdits()));\n",
        lift(&["stagedEdits"])
    );
    let out = run_node(&script);
    assert_eq!(out.trim(), r#"[{"id":"aaaaaaa","field":"status","value":"done"},{"id":"bbbbbbb","field":"priority","value":"low"}]"#, "{out}");
}

/// A refused batch has two things to say and the page has to say both: what stopped it, in the
/// engine's own words, and how much had already landed — because each operation is its own
/// commit, so "it failed" and "nothing happened" are different sentences.
#[test]
fn a_refused_batch_says_what_stopped_it_and_what_had_already_landed() {
    if !have_node() {
        return;
    }
    let script = format!(
        "{}\n\
         console.log(JSON.stringify([\n\
           applyMessage({{ ok: false, applied: [], error: \"unknown priority 'urgent'\" }}),\n\
           applyMessage({{ ok: false, applied: ['#a moved'], error: 'refused' }}),\n\
           applyMessage({{ ok: false, applied: ['#a moved', '#b moved'], error: 'refused' }}),\n\
           applyMessage({{ ok: false, applied: [] }}),\n\
         ]));\n",
        lift(&["applyMessage"])
    );
    let out = run_node(&script);
    // Nothing landed: the diagnostic alone, with no preamble claiming a partial write.
    assert!(out.contains(r#""unknown priority 'urgent'""#), "{out}");
    assert!(out.contains("1 operation applied before it stopped — refused"), "{out}");
    assert!(out.contains("2 operations applied before it stopped — refused"), "{out}");
    // A response carrying no `error` at all still says something rather than "undefined".
    assert!(out.contains("the write was refused"), "{out}");
}

/// A staged edit that reality caught up with is not an edit any more.
///
/// Somebody else — or you, in a terminal — may make the very change that is staged here. After
/// a live re-render, keeping it would leave a command in the panel that would now do nothing;
/// `setEdit` already treats "same as the issue" as not an edit, and this applies that same rule
/// to the issue having moved rather than the field having been set back.
#[test]
fn a_live_re_render_forgets_the_staged_edits_reality_caught_up_with() {
    if !have_node() {
        return;
    }
    let script = format!(
        "const byId = {{\n\
           a: {{ id: 'a', status: 'done', priority: 'high' }},\n\
           b: {{ id: 'b', status: 'backlog', priority: 'low' }},\n\
         }};\n\
         const state = {{ edits: {{\n\
           'a::status': 'done',\n\
           'a::priority': 'low',\n\
           'b::status': 'in-progress',\n\
           'gone::status': 'done',\n\
         }} }};\n\
         {}\n\
         dropSettledEdits();\n\
         console.log(JSON.stringify(Object.keys(state.edits).sort()));\n",
        lift(&["dropSettledEdits"])
    );
    let out = run_node(&script);
    // `a::status` says what the issue now is, and `gone` is not in the tracker any more. The
    // two that still ask for something survive.
    assert_eq!(out.trim(), r#"["a::priority","b::status"]"#, "{out}");
}

/// The index every renderer reads is rebuilt **in place**. A live re-render replaces the
/// model, and a renderer that had closed over the old object would go on drawing the tracker
/// as it was — silently, and only in whichever views happened to capture it.
#[test]
fn the_issue_index_is_rebuilt_in_place_rather_than_replaced() {
    if !have_node() {
        return;
    }
    let script = format!(
        "const DATA = {{ issues: [{{ id: 'a' }}, {{ id: 'b' }}] }};\n\
         {}\n\
         const captured = byId;\n\
         DATA.issues = [{{ id: 'b' }}, {{ id: 'c' }}];\n\
         reindex();\n\
         console.log(JSON.stringify([captured === byId, Object.keys(byId).sort()]));\n",
        lift(&["byId", "reindex"])
    );
    let out = run_node(&script);
    assert_eq!(out.trim(), r#"[true,["b","c"]]"#, "{out}");
}

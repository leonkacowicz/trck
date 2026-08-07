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

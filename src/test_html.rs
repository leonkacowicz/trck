//! What the page is made of, and what it must never contain.
//!
//! Its own file, the way `test_graph.rs` and `test_index.rs` are: `html.rs` is the schema the
//! page reads and this is the list of promises it makes, and the two are consulted for
//! different reasons.

// Tests assert; that is their job. The crate denies unwrap/expect/panic because a
// malformed tracker must produce a diagnostic rather than a stack trace, but a test
// that cannot panic cannot fail.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use crate::html::*;
use crate::json::Json;

#[test]
fn the_assets_are_compiled_in() {
    assert!(CSS.len() > 10_000, "stylesheet looks truncated");
    assert!(APP_JS.len() > 40_000, "script looks truncated");
    assert!(SHELL.contains("<div"), "shell looks wrong");
}

#[test]
fn the_page_references_nothing_external() {
    // The whole point: a file you can open, mail, or commit. A CDN script or a
    // remote font would make it a page that only works online.
    //
    // The test is what gets *fetched*, not what merely contains a URL: `SVGNS` is
    // `http://www.w3.org/2000/svg`, an XML namespace identifier that
    // `createElementNS` requires and nothing ever requests. A blanket "no http"
    // rule would fail on it and teach the next reader to weaken the check.
    for asset in [CSS, SHELL, APP_JS] {
        for needle in ["src=\"http", "href=\"http", "<link", "@import", "//cdn"] {
            assert!(!asset.contains(needle), "asset fetches something external: {needle}");
        }
    }
    assert!(APP_JS.contains("http://www.w3.org/2000/svg"), "the SVG namespace should still be here — if it went, this test lost its point");
}

#[test]
fn the_island_escapes_anything_that_could_break_out() {
    // An issue body is arbitrary text. Without this it could close the script tag.
    let model = Json::String("</script><img src=x onerror=alert(1)> & \u{2028}".into());
    let island = json_island(&model);
    assert!(!island.contains('<'), "{island}");
    assert!(!island.contains('>'), "{island}");
    assert!(!island.contains('&'), "{island}");
    assert!(!island.contains('\u{2028}'), "{island}");
}

#[test]
fn the_title_is_html_escaped() {
    assert_eq!(escape_html("a<b>&c"), "a&lt;b&gt;&amp;c");
}

#[test]
fn the_shell_height_is_derived_rather_than_a_constant() {
    // `main` was once `height: calc(100vh - 92px)`, where 92px was a hand-measured
    // guess at the topbar plus the filter bar. Every way that guess can be wrong —
    // a different font metric, a zoom level, a filter bar wrapping to two lines,
    // a view that hides the facets — made the page taller or shorter than the
    // window, which a reader saw as a document scrollbar that scrolled nothing.
    //
    // Nothing here can check layout: CSS is a string to this crate and there is no
    // engine to lay it out. What it can check is that the constant has not come
    // back, because the constant is the bug. A viewport-tall flex column with a
    // shrinkable `main` is the shape that needs no number.
    //
    // Comments are stripped first, because the rule this guards is explained in one —
    // and a stylesheet that merely *mentions* the old declaration is not the bug.
    let rules: String = CSS.split("/*").map(|part| part.split_once("*/").map_or(part, |(_, tail)| tail)).collect();
    assert!(!rules.contains("calc(100vh -") && !rules.contains("calc(100dvh -"), "the shell is subtracting a hardcoded chrome height again");
    for needle in ["flex-direction: column", "height: 100dvh", "min-height: 0"] {
        assert!(rules.contains(needle), "the viewport-tall column lost `{needle}`");
    }
}

//! `trck html` — the tracker as one self-contained HTML page.
//!
//! Most of this file is three `include_str!`s. The stylesheet, the shell and the
//! application script are static assets compiled in verbatim; the actual work is
//! building the JSON data island the script reads, which is why the schema below is
//! written out field by field rather than derived.
//!
//! **Self-contained is the point.** No network requests, no external references,
//! nothing to serve — the output is a file you can open, mail, or commit. Every derived
//! value the page needs (readiness, the demand cone, rollup percentages, the
//! shortest-unique-id prefix) is computed here by the engine rather than re-derived in
//! JavaScript, so the page and the CLI can never disagree about what the tracker says.

use crate::discovery::Ctx;
use crate::graph::Graph;
use crate::issue::Issue;
use crate::json::Json;
use crate::render::unique_prefix_lens;
use crate::{config, summary};

/// The stylesheet and the script, compiled in.
///
/// Visible past this module because `serve` answers `/app.css` and `/app.js` from these very
/// constants — the point being that a live process can never serve an asset out of whatever
/// working tree it was launched in, which may be on another branch entirely.
pub(crate) const CSS: &str = include_str!("../assets/app.css");
const SHELL: &str = include_str!("../assets/shell.html");
pub(crate) const APP_JS: &str = include_str!("../assets/app.js");

fn s(v: &str) -> Json {
    Json::String(v.to_string())
}

fn opt(v: Option<&String>) -> Json {
    v.map_or(Json::Null, |x| s(x))
}

fn num(v: i64) -> Json {
    Json::Number(v.to_string())
}

fn ids(v: &[String]) -> Json {
    Json::Array(v.iter().map(|x| s(x)).collect())
}

/// The points-weighted rollup of a parent, or null for a leaf.
fn progress(g: &Graph, id: &str) -> Json {
    if g.is_leaf(id) {
        return Json::Null;
    }
    let (pdone, ptotal, ndone, ntotal) = g.leaf_rollup(id);
    let pct = if ptotal == 0 { 0 } else { (200 * pdone + ptotal) / (2 * ptotal) };
    Json::Object(vec![
        ("pct".into(), num(pct)),
        ("done_points".into(), num(pdone)),
        ("total_points".into(), num(ptotal)),
        ("done_count".into(), num(i64::try_from(ndone).unwrap_or(0))),
        ("total_count".into(), num(i64::try_from(ntotal).unwrap_or(0))),
    ])
}

/// One issue, as the page's schema.
///
/// Everything derived is computed here rather than in the script: the page ships the
/// same order `trck ready` prints and re-derives no cone maths in JavaScript.
/// `demand_source` is null exactly when the issue already leads its own cone, which is
/// also exactly when no `↑priority` marker is warranted — so the client renders the
/// marker if and only if the field is set, with no rank comparison of its own.
fn issue_json(g: &Graph, ctx: &Ctx, r: &Issue) -> Json {
    // A body that has gone missing is `check`'s finding to report, not the page's to
    // refuse over: an issue with no prose still has a row worth rendering.
    let body = ctx.read_body(r).unwrap_or_default();
    let demand: Vec<Json> = g.demand_vector(&r.id).into_iter().map(|n| num(i64::try_from(n).unwrap_or(0))).collect();
    Json::Object(vec![
        ("id".into(), s(&r.id)),
        ("title".into(), s(&r.title)),
        ("status".into(), s(&r.status)),
        ("priority".into(), s(&r.priority)),
        ("kind".into(), r.extra.get("kind").and_then(|v| v.as_str()).map_or(Json::Null, s)),
        ("points".into(), num(r.points)),
        ("labels".into(), ids(&r.labels)),
        ("resolution".into(), opt(r.resolution.as_ref())),
        ("parent".into(), opt(r.parent.as_ref())),
        ("children".into(), ids(g.children_of(&r.id))),
        ("requires".into(), ids(&g.requires_of(&r.id))),
        ("dependents".into(), ids(g.dependents_of(&r.id))),
        ("blocked".into(), Json::Bool(g.is_blocked(&r.id))),
        ("ready".into(), Json::Bool(g.is_ready(&r.id))),
        ("demand".into(), Json::Array(demand)),
        ("demand_source".into(), g.demand_source(&r.id).map_or(Json::Null, |x| s(&x))),
        ("leaf".into(), Json::Bool(g.is_leaf(&r.id))),
        ("terminal".into(), Json::Bool(config::is_terminal(&r.status))),
        ("progress".into(), progress(g, &r.id)),
        ("created".into(), opt(r.created.as_ref())),
        ("started".into(), opt(r.started.as_ref())),
        ("closed".into(), opt(r.closed.as_ref())),
        ("spec".into(), opt(r.spec.as_ref())),
        ("review_url".into(), opt(r.review_url.as_ref())),
        ("body".into(), s(&body)),
    ])
}

/// The command prefix the page's copy-to-clipboard commands are built from: a global
/// `trck` when one is on PATH, else a repo-relative path to the running binary.
fn default_cmd() -> String {
    let on_path = std::env::var_os("PATH").is_some_and(|paths| std::env::split_paths(&paths).any(|d| d.join("trck").is_file()));
    if on_path {
        return "trck".to_string();
    }
    let Ok(exe) = std::env::current_exe() else {
        return "trck".to_string();
    };
    std::env::current_dir()
        .ok()
        .and_then(|cwd| exe.strip_prefix(&cwd).ok().map(std::path::Path::to_path_buf))
        .map_or_else(|| exe.display().to_string(), |rel| format!("./{}", rel.display()))
}

/// The whole model the page is built from.
pub(crate) fn build_model(ctx: &Ctx, g: &Graph, cmd: Option<&str>) -> Json {
    // The project the tracker belongs to: the directory holding it. Deliberately not
    // `update.repo` — that is the engine's release channel, and it would title every
    // consumer's page with trck's own upstream slug.
    let (repo, tracker) = ctx.labels();

    // Each id's shortest-unique-prefix length, computed with the helper the CLI uses so
    // the page's highlight matches `trck list` exactly.
    let plen = unique_prefix_lens(g.rows.iter().map(|r| r.id.as_str()));
    let mut issues: Vec<Json> = Vec::new();
    let mut edges: Vec<Json> = Vec::new();
    for r in &g.rows {
        let mut obj = issue_json(g, ctx, r);
        if let Json::Object(pairs) = &mut obj {
            pairs.push(("plen".into(), num(i64::try_from(plen.get(&r.id).copied().unwrap_or(1)).unwrap_or(1))));
        }
        issues.push(obj);
        // Authored dependency edges, blocker -> blocked. Containment is deliberately
        // excluded: the graph view draws ordering constraints, not the hierarchy.
        for b in g.requires_of(&r.id) {
            edges.push(Json::Object(vec![("from".into(), s(&b)), ("to".into(), s(&r.id))]));
        }
    }
    let mut roots: Vec<String> = g.rows.iter().filter(|r| r.parent.is_none()).map(|r| r.id.clone()).collect();
    roots.sort();

    // The vocabulary is fixed in the engine, but the page still ships it: the board
    // draws a column per status and the facet bar a box per status, and neither should
    // hardcode names the engine owns.
    let statuses: Vec<Json> =
        config::STATUSES.iter().map(|n| Json::Object(vec![("name".into(), s(n)), ("terminal".into(), Json::Bool(config::is_terminal(n)))])).collect();
    Json::Object(vec![
        ("repo".into(), s(&repo)),
        ("tracker".into(), s(&tracker)),
        (
            "config".into(),
            Json::Object(vec![
                ("statuses".into(), Json::Array(statuses)),
                ("priorities".into(), Json::Array(config::PRIORITIES.iter().map(|p| s(p)).collect())),
                ("cmd".into(), s(&cmd.map_or_else(default_cmd, str::to_string))),
            ]),
        ),
        ("issues".into(), Json::Array(issues)),
        ("edges".into(), Json::Array(edges)),
        ("roots".into(), Json::Array(roots.iter().map(|r| s(r)).collect())),
    ])
}

/// Escape the data island so an issue body can never break out of the `<script>` tag or
/// inject markup. The two line separators are escaped because they terminate a
/// JavaScript string literal even though JSON allows them raw.
fn json_island(model: &Json) -> String {
    model.to_json().replace('<', "\\u003c").replace('>', "\\u003e").replace('&', "\\u0026").replace('\u{2028}', "\\u2028").replace('\u{2029}', "\\u2029")
}

fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// A `Ctx` to one self-contained HTML page.
pub(crate) fn render_html(ctx: &Ctx, g: &Graph, cmd: Option<&str>) -> String {
    let model = build_model(ctx, g, cmd);
    let repo = model.get("repo").and_then(Json::as_str).unwrap_or_default().to_string();
    let title = escape_html(&format!("trck · {repo}"));
    format!(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n\
         <meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <title>{title}</title>\n\
         <style>\n{CSS}\n</style>\n</head>\n<body>\n\
         {SHELL}\
         <script type=\"application/json\" id=\"trck-data\">{}</script>\n\
         <script>\n{APP_JS}\n</script>\n\
         </body>\n</html>\n",
        json_island(&model)
    )
}

/// Write the page. The default output path sits beside the index, so `trck html` in a
/// repo produces `issues/issues.html` with nothing else to decide.
pub(crate) fn cmd_html(ctx: &Ctx, out: Option<&str>, cmd: Option<&str>) -> Result<String, String> {
    let rows = crate::verbs::load_rows(ctx)?;
    let g = Graph::new(rows);
    let html = render_html(ctx, &g, cmd);
    // Beside the index when there is one. A ref-backed tracker has no directory to sit
    // beside, so the page lands where the command was run instead of nowhere.
    let path = match out {
        Some(spec) => std::path::PathBuf::from(spec),
        None => ctx.dir().map_or_else(|_| std::path::PathBuf::from("issues.html"), |d| d.join("issues.html")),
    };
    if path.as_os_str() == "-" {
        return Ok(html);
    }
    crate::verbs::write_file(&path, &html)?;
    Ok(format!("wrote {} ({} issues)", path.display(), g.rows.len()))
}

/// Where the summary module's helper lives, re-exported so the page and `SUMMARY.md`
/// cannot drift on what an issue's file is called.
pub(crate) fn body_filename(r: &Issue) -> String {
    summary::filename(r)
}

#[cfg(test)]
mod tests {
    // Tests assert; that is their job. The crate denies unwrap/expect/panic because a
    // malformed tracker must produce a diagnostic rather than a stack trace, but a test
    // that cannot panic cannot fail.
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

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
}

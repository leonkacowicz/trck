//! `trck html` — the tracker as one self-contained HTML page.
//!
//! Most of this file is three `include_str!`s. The stylesheet, the shell and the
//! application script are static assets compiled in verbatim; the actual work is
//! building the JSON data island the script reads, which is why the schema below is
//! written out field by field rather than derived.
//!
//! **Self-contained is the point.** No external references, nothing to serve — the output
//! is a file you can open, mail, or commit. Every derived value the page needs (readiness,
//! the demand cone, rollup percentages, the shortest-unique-id prefix) is computed here by
//! the engine rather than re-derived in JavaScript, so the page and the CLI can never
//! disagree about what the tracker says.
//!
//! The script's one possible request is same-origin and only when told: `serve` sets
//! `config.live` so Apply can post staged edits back. A file says `live: false` and fetches
//! nothing, which is what keeps "open it over `file://`" true.

use crate::discovery::Ctx;
use crate::graph::Graph;
use crate::issue::Issue;
use crate::json::Json;
use crate::render::unique_prefix_lens;
use crate::{config, summary};

/// The stylesheet and the script, compiled in. Visible past this module because `serve`
/// answers `/app.css` and `/app.js` from these very constants — so a live process can never
/// serve an asset out of the working tree it was launched in, which may be on another branch.
pub(crate) const CSS: &str = include_str!("../assets/app.css");
pub(crate) const SHELL: &str = include_str!("../assets/shell.html");
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

/// What a particular *rendering* is, as opposed to what the tracker says.
///
/// Three facts that travel together because they answer the same question — which page is this,
/// and what can it do — and because a `build_model` taking them loose is one whose call sites
/// pass three positional values, two of them `Option<&str>` and neither named at the call.
pub(crate) struct Page<'a> {
    /// The command prefix the page's copy-to-clipboard commands are built from, or `None` to
    /// work one out.
    pub(crate) cmd: Option<&'a str>,
    /// Whether a process is behind this page, and so whether staged edits can be applied from
    /// it. Told, not sniffed: to `location.protocol` any statically-served page looks live.
    pub(crate) live: bool,
    /// What this rendering was made from, so a page reconnecting to the event stream can say
    /// where it is and be caught up if it is behind.
    pub(crate) version: Option<&'a str>,
}

/// The whole model the page is built from.
pub(crate) fn build_model(ctx: &Ctx, g: &Graph, page: &Page) -> Json {
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
                ("cmd".into(), s(&page.cmd.map_or_else(default_cmd, str::to_string))),
                ("live".into(), Json::Bool(page.live)),
                ("version".into(), page.version.map_or(Json::Null, s)),
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
pub(crate) fn json_island(model: &Json) -> String {
    model.to_json().replace('<', "\\u003c").replace('>', "\\u003e").replace('&', "\\u0026").replace('\u{2028}', "\\u2028").replace('\u{2029}', "\\u2029")
}

pub(crate) fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// A `Ctx` to one self-contained HTML page.
pub(crate) fn render_html(ctx: &Ctx, g: &Graph, page: &Page) -> String {
    let model = build_model(ctx, g, page);
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
    // A written file has no process behind it, whatever it is later served by — and so no
    // version to be caught up from.
    let html = render_html(ctx, &g, &Page { cmd, live: false, version: None });
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

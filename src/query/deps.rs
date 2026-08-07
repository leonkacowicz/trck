//! `deps`: one issue's dependency line, or the whole graph, drawn as a gutter DAG.
//!
//! Split out of the other read verbs for the reason `list` was: choosing *what to draw* and
//! *drawing it* are different questions. The choosing half is the weight here — four ways to
//! arrive at an id set, three of which can answer the verb outright without a graph ever
//! being drawn — so it lives behind one type that holds what all of them need.

use crate::config::is_terminal;
use crate::discovery::Ctx;
use crate::graph::Graph;
use crate::gutter;
use crate::issue::Issue;
use crate::json::Json;
use crate::render::{LANE_PALETTE, gutter, hl_id, lane_palette_index, paint, unique_prefix_lens};
use crate::verbs::{load_rows, resolve_ref};
use std::collections::{BTreeMap, BTreeSet};

/// Options `deps` accepts.
#[allow(clippy::struct_excessive_bools, reason = "mirrors the CLI flags one-to-one")]
#[derive(Default)]
pub(crate) struct DepsOpts<'a> {
    pub(crate) root: Option<&'a str>,
    pub(crate) requires: bool,
    pub(crate) blocks: bool,
    pub(crate) full: bool,
    pub(crate) omit_done: bool,
    pub(crate) include_done_chains: bool,
    pub(crate) fanout: bool,
}

/// `deps --json`: one issue's two cones, as `{requires, blocks}`.
///
/// Needs an id. The whole-graph view is an edge list — a different shape entirely — and
/// silently returning one under the same key names would be worse than refusing.
///
/// Rows are emitted in index order rather than the order the cone walk happens to produce:
/// the walk works off a set, so its iteration order is not something a golden file could
/// survive.
pub(crate) fn cmd_deps_json(ctx: &Ctx, opts: &DepsOpts) -> Result<String, String> {
    let rows = load_rows(ctx)?;
    let Some(token) = opts.root else {
        return Err("deps --json needs an issue id (the whole-graph view is an edge \
                    list, a different shape from one issue's cones)"
            .into());
    };
    let iid = resolve_ref(&rows, token)?;
    let g = Graph::new(rows);
    let cone = |up: bool, down: bool| -> Vec<Json> {
        let line = g.dependency_line(&iid, up, down);
        g.rows.iter().filter(|r| r.id != iid && line.contains(&r.id)).map(Issue::to_full).collect()
    };
    Ok(Json::Object(vec![("requires".into(), Json::Array(cone(true, false))), ("blocks".into(), Json::Array(cone(false, true)))]).to_json_pretty())
}

pub(crate) fn cmd_deps(ctx: &Ctx, opts: &DepsOpts) -> Result<String, String> {
    let rows = load_rows(ctx)?;
    let root = opts.root.map(|t| resolve_ref(&rows, t)).transpose()?;
    let abbrev = unique_prefix_lens(rows.iter().map(|r| r.id.as_str()));
    let deps = Deps { g: Graph::new(rows), root, abbrev, opts };
    let ids = match deps.select()? {
        Selection::Answer(text) => return Ok(text),
        Selection::Ids(ids) => ids,
    };
    Ok(deps.draw(&deps.done_filtered(&ids)))
}

/// What selecting the ids came to: a set still to be drawn, or the verb's whole answer.
///
/// Three of the four selection paths can end the verb on their own — an issue with no edges
/// at all, that same issue hidden by `--omit-done`, a tracker with no authored edges
/// anywhere — and each has its own text. Returning that text is what keeps them from being
/// early returns threaded back through the drawing half.
enum Selection {
    Ids(BTreeSet<String>),
    Answer(String),
}

/// Everything both halves of `deps` need, resolved once: the graph, the focal issue if one
/// was named, how far ids can be abbreviated, and the flags.
struct Deps<'a> {
    g: Graph,
    root: Option<String>,
    abbrev: BTreeMap<String, usize>,
    opts: &'a DepsOpts<'a>,
}

impl Deps<'_> {
    /// The ids to draw, or the answer to print instead — and the one place the option
    /// combination is judged, because `--requires`/`--blocks` scope a cone and there is no
    /// cone to scope until it is known whether an issue was named.
    fn select(&self) -> Result<Selection, String> {
        let Some(id) = self.root.clone() else {
            if self.opts.requires || self.opts.blocks {
                return Err("deps: --requires/--blocks scope one issue's graph — pass an issue id".into());
            }
            let ids = gutter::overview_ids(&self.g);
            return Ok(if ids.is_empty() { Selection::Answer("no dependencies recorded yet".into()) } else { Selection::Ids(ids) });
        };
        if !self.has_edges(&id) {
            return self.lone(&id).map(Selection::Answer);
        }
        Ok(Selection::Ids(if self.opts.full { self.component(&id) } else { self.line(&id) }))
    }

    /// Whether the issue sits on any edge this view would draw — authored or containment.
    fn has_edges(&self, id: &str) -> bool {
        !self.g.requires_of(id).is_empty()
            || !self.g.dependents_of(id).is_empty()
            || !self.g.children_of(id).is_empty()
            || self.g.get(id).and_then(|r| r.parent.clone()).is_some()
    }

    /// The answer for an issue on no edges at all: its bare label — or nothing, when it is
    /// done and `--omit-done` asked for done work to go away.
    fn lone(&self, id: &str) -> Result<String, String> {
        if self.opts.omit_done && self.g.get(id).is_some_and(|r| is_terminal(&r.status)) {
            return Ok(String::new());
        }
        let Some(row) = self.g.get(id) else {
            return Err(format!("no issue matching '{id}'"));
        };
        Ok(format!("{}  (no dependencies)", self.label(row, true)))
    }

    /// `--full`: the focal node's whole component, computed over *every* issue — not over
    /// the overview set, which drops the components the bare view suppresses and could
    /// therefore lose the focal node itself.
    fn component(&self, id: &str) -> BTreeSet<String> {
        let all: BTreeSet<String> = self.g.rows.iter().map(|r| r.id.clone()).collect();
        let edges = gutter::drawn_edges(&self.g, &all, false, false);
        gutter::components(&all, &edges).into_iter().find(|c| c.iter().any(|m| m == id)).unwrap_or_default().into_iter().collect()
    }

    /// One issue's cone. Neither flag shows both directions; one flag scopes to that one.
    fn line(&self, id: &str) -> BTreeSet<String> {
        let up = self.opts.requires || !self.opts.blocks;
        let down = self.opts.blocks || !self.opts.requires;
        self.g.dependency_line(id, up, down)
    }

    /// Done filtering, which is display-only: fully terminal components disappear from the
    /// whole-graph view, never from an issue's own line.
    fn done_filtered(&self, ids: &BTreeSet<String>) -> BTreeSet<String> {
        gutter::filter_done(&self.g, ids, self.opts.omit_done, self.opts.include_done_chains, self.root.is_none())
    }

    /// The graph itself: a gutter column padded to a common width, then each row's label.
    fn draw(&self, ids: &BTreeSet<String>) -> String {
        let rendered = gutter::render_graph(&self.g, ids, self.opts.fanout);
        let width = rendered.iter().flatten().map(|(_, gut, _)| gut.chars().count()).max().unwrap_or(0);
        let mut out: Vec<String> = Vec::new();
        for row in &rendered {
            let Some((iid, gut, owners)) = row else {
                out.push(String::new());
                continue;
            };
            let Some(issue) = self.g.get(iid) else { continue };
            let focal = self.root.as_deref() == Some(iid.as_str());
            let painted = paint_lanes(gut, owners);
            out.push(format!("{}{painted}{}  {}", self.margin(focal), " ".repeat(width - gut.chars().count()), self.label(issue, focal)));
        }
        out.join("\n")
    }

    /// A left-margin caret marks the focal row; a blank 2-column margin on every other row
    /// keeps the graph aligned. The whole-graph view has no focal node, so it has no margin.
    fn margin(&self, focal: bool) -> String {
        match (&self.root, focal) {
            (None, _) => String::new(),
            (Some(_), true) => format!("{} ", paint("▸", &["bold"])),
            (Some(_), false) => "  ".to_string(),
        }
    }

    /// One node's label in the graph: id, status icon, title, and a derived epic marker.
    ///
    /// `·epic·` comes from the hierarchy, not from a stored kind — an issue with children
    /// *is* an epic, and a declared marker only drifts from that.
    fn label(&self, r: &Issue, focal: bool) -> String {
        let tag = if self.g.children_of(&r.id).is_empty() { String::new() } else { " ·epic·".to_string() };
        let labels = if r.labels.is_empty() { String::new() } else { paint(&format!(" [{}]", r.labels.join(" ")), &["dim"]) };
        let emph: &[&str] = if focal { &["bold"] } else { &[] };
        // Status only, never the ready glyph: this view answers "what is waiting on
        // what", and marking the roots of the unblocked chains would restate what the
        // gutter beside them already draws.
        let (glyph, codes) = gutter(&r.status, false);
        format!("{} {} {}{tag}{labels}", hl_id(&r.id, Some(&self.abbrev), true), paint(glyph, codes), paint(&r.title, emph))
    }
}

/// A gutter row with each lane coloured by a rotating palette keyed on the id it heads to,
/// so a lane keeps one hue for its whole descent and can be traced through crossings. An
/// inferred containment edge is dimmed *on top of* its hue — weight, not colour, marks it as
/// structure — since box-drawing has no dashed corner to distinguish it by shape. The node's
/// own bullet (`●`) is bold rather than palette-coloured.
fn paint_lanes(gut: &str, owners: &[gutter::LaneOwner]) -> String {
    gut.chars()
        .zip(owners.iter())
        .map(|(ch, owner)| {
            if ch == '●' {
                return paint("●", &["bold"]);
            }
            match owner {
                None => ch.to_string(),
                Some((id, kind)) => {
                    let mut codes = vec![LANE_PALETTE[lane_palette_index(id)]];
                    if *kind == gutter::EdgeKind::Child {
                        codes.insert(0, "dim");
                    }
                    paint(&ch.to_string(), &codes)
                },
            }
        })
        .collect()
}

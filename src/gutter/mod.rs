//! The dependency DAG as a lazygit-style gutter: one row per node, every merge and fork
//! drawn on its own node's row as box-drawing corners rather than on blank edge rows.
//!
//! A lane opens at a prerequisite's row and closes at its dependent's, so each lane
//! column carries exactly one edge — which is why the edge *kind* rides along with the
//! lane and an inferred containment edge can be drawn differently from an authored one.
//!
//! Four things happen before any drawing, and the order matters. Each is a module here,
//! and this one is only the entry points that string them together:
//!
//! 1. [`edges`] **builds the drawn edge set** over exactly the ids being drawn, and
//!    [`reduce`] drops what a longer path already implies — so an edge is only ever
//!    dropped in favour of a path that is itself on screen.
//! 2. [`components`] splits the result into separable blocks, each drawn as one contiguous
//!    run of rows.
//! 3. [`order`] puts prerequisites first, tie-broken by locality, and [`shorten`] then
//!    slides nodes to close the lanes locality left open — because locality is a local
//!    rule: it settles each step without seeing what the choice costs further down, and
//!    strands a root whose only dependent is far below.
//! 4. [`rows`] gives each node a lane and [`canvas`] draws the cells.

use crate::config::is_terminal;
use crate::graph::Graph;
use std::collections::{BTreeMap, BTreeSet};

mod canvas;
mod components;
mod edges;
mod order;
mod reduce;
mod rows;
mod shorten;

pub(crate) use components::components;
pub(crate) use edges::drawn_edges;

/// The kind of a drawn edge. Authored dependencies and inferred containment are drawn
/// differently, so the lane carries which it is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum EdgeKind {
    /// An authored `depends_on`.
    Dep,
    /// A parent waiting on a child. Nobody authors these; they follow from containment.
    Child,
    /// A dependency lifted from an ancestor.
    Inherited,
}

/// The drawn edge set: source -> its targets, each with the kind of edge.
pub(crate) type Edges = BTreeMap<String, Vec<(String, EdgeKind)>>;

/// Which edge a gutter cell belongs to, for colouring. `None` where nothing owns it.
pub(crate) type LaneOwner = Option<(String, EdgeKind)>;

/// One rendered row: the issue id, the gutter, and a per-character lane owner.
pub(crate) type Row = (String, String, Vec<LaneOwner>);

/// Render the DAG over `ids`, grouped by component with a `None` separator between
/// groups.
///
/// The edge set is built and reduced here, over exactly the ids being drawn. Doing it at
/// this point rather than at the caller is what makes the ordering trap impossible:
/// `ids` has already been done-filtered, so an edge can never be dropped in favour of a
/// path through a node that is not rendered — which would leave its endpoints looking
/// unrelated.
pub(crate) fn render_graph(g: &Graph, ids: &BTreeSet<String>, fanout: bool) -> Vec<Option<Row>> {
    let edges = drawn_edges(g, ids, true, fanout);
    let unreduced = drawn_edges(g, ids, false, fanout);
    let mut out: Vec<Option<Row>> = Vec::new();
    for comp in components(ids, &unreduced) {
        if !out.is_empty() {
            out.push(None);
        }
        out.extend(rows::component_rows(&comp, &edges).into_iter().map(Some));
    }
    out
}

/// The id set for the bare `deps` view: every component holding at least one *authored*
/// edge, taken whole.
///
/// Containment edges connect nearly the whole forest, so "every issue touching an edge"
/// would match almost everything and turn this view into `list`. Selecting by authored
/// edges keeps it about ordering constraints. Components are kept or dropped *whole*: a
/// parent shown without some of its children would misreport what it is waiting on,
/// which is precisely the question this view exists to answer.
pub(crate) fn overview_ids(g: &Graph) -> BTreeSet<String> {
    let all: BTreeSet<String> = g.rows.iter().map(|r| r.id.clone()).collect();
    let edges = drawn_edges(g, &all, false, false);
    let mut keep = BTreeSet::new();
    for comp in components(&all, &edges) {
        let members: BTreeSet<&str> = comp.iter().map(String::as_str).collect();
        let authored = comp.iter().any(|i| g.get(i).is_some_and(|r| r.depends_on.iter().any(|d| members.contains(d.as_str()))));
        if authored {
            keep.extend(comp);
        }
    }
    keep
}

/// Which display-only done filtering to apply. Three independent switches rather than one
/// mode, because the view and the flags decide them separately.
pub(crate) struct DoneFilter {
    /// Drop terminal issues wherever they appear.
    pub(crate) omit_done: bool,
    /// Keep finished components that `hide_chains` would otherwise drop.
    pub(crate) include_chains: bool,
    /// Whole-graph view only: a component with nothing left in it answers nothing the
    /// view is asking. A view rooted at one issue is showing that issue's line, and drops
    /// no part of it.
    pub(crate) hide_chains: bool,
}

/// Display-only done filtering.
pub(crate) fn filter_done(g: &Graph, ids: &BTreeSet<String>, filter: &DoneFilter) -> BTreeSet<String> {
    let mut kept = ids.clone();
    if filter.hide_chains && !filter.include_chains {
        kept = live_components(g, &kept);
    }
    if filter.omit_done {
        kept.retain(|i| !terminal(g, i));
    }
    kept
}

/// Is this issue in a status the workflow treats as finished?
fn terminal(g: &Graph, id: &str) -> bool {
    g.get(id).is_some_and(|r| is_terminal(&r.status))
}

/// `ids` minus every component whose every issue is terminal.
///
/// Removing done nodes shrinks the id set, so the components are recomputed over the
/// remaining subgraph rather than reused — otherwise a synthetic edge would appear across
/// the omitted nodes.
fn live_components(g: &Graph, ids: &BTreeSet<String>) -> BTreeSet<String> {
    let edges = drawn_edges(g, ids, false, false);
    let mut kept = ids.clone();
    for comp in components(ids, &edges) {
        if comp.iter().all(|i| terminal(g, i)) {
            for i in comp {
                kept.remove(&i);
            }
        }
    }
    kept
}

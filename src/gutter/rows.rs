//! Lane assignment: which column each node's bullet sits in, and which lanes open and close
//! around it. [`super::canvas`] turns one of these into characters.

use super::canvas::Canvas;
use super::order::{Topo, topo};
use super::{EdgeKind, Edges, LaneOwner, Row};

/// One row's lane bookkeeping: the lanes above and below it, the node's own column, and
/// which columns close into the node and which open beneath it.
///
/// It travels as one value because the drawing needs all five at once, and because
/// `arriving` and `started` are only meaningful against the `top` and `bottom` they were
/// computed from.
struct Transition {
    top: Vec<LaneOwner>,
    bottom: Vec<LaneOwner>,
    /// The node's own column.
    pos: usize,
    /// Columns whose lane ends at this node.
    arriving: Vec<usize>,
    /// Columns whose lane begins at this node.
    started: Vec<usize>,
}

impl Transition {
    /// Close every lane arriving at `n` and pick the node's column: the leftmost arriving
    /// lane, or the first free column when nothing arrives.
    fn arrive(lanes: &[LaneOwner], n: &str) -> Transition {
        let top = lanes.to_vec();
        let arriving: Vec<usize> = top.iter().enumerate().filter(|(_, t)| t.as_ref().is_some_and(|(d, _)| d == n)).map(|(c, _)| c).collect();
        let pos = arriving.first().copied().unwrap_or_else(|| top.iter().position(Option::is_none).unwrap_or(top.len()));
        let mut bottom = top.clone();
        while bottom.len() <= pos {
            bottom.push(None);
        }
        for c in &arriving {
            bottom[*c] = None;
        }
        bottom[pos] = None;
        Transition { top, bottom, pos, arriving, started: Vec::new() }
    }

    /// Open a lane beneath the node for one dependent. The first takes the node's own
    /// column; the rest go to [`Self::nearest_free`].
    fn open(&mut self, lane: LaneOwner, first: bool) {
        let c = if first { self.pos } else { self.nearest_free() };
        self.bottom[c] = lane;
        self.started.push(c);
    }

    /// The free column *nearest* the node, not the leftmost gap: the same lane count, but a
    /// shorter horizontal bridge and fewer crossings. Ties go to the lower column. Appends a
    /// column when the row is full.
    fn nearest_free(&mut self) -> usize {
        let pos = self.pos;
        let free = self.bottom.iter().enumerate().filter(|(_, t)| t.is_none()).map(|(c, _)| c);
        let c = free.min_by_key(|c| (c.abs_diff(pos), *c)).unwrap_or(self.bottom.len());
        if c == self.bottom.len() {
            self.bottom.push(None);
        }
        c
    }

    /// Draw the row: the lanes passing through it, then every lane that arrives at or starts
    /// from the node joined across to it.
    fn draw(&self, n: &str) -> Row {
        let width = self.top.len().max(self.bottom.len()).max(self.pos + 1);
        let mut canvas = Canvas::new(width, self.pos);
        canvas.through(&self.top, &self.arriving);
        for a in &self.arriving {
            canvas.connect(*a, &self.top.get(*a).cloned().flatten(), 'U');
        }
        for b in &self.started {
            canvas.connect(*b, &self.bottom.get(*b).cloned().flatten(), 'D');
        }
        canvas.render(n)
    }
}

/// Render one connected component, one row per node.
pub(super) fn component_rows(comp: &[String], edges: &Edges) -> Vec<Row> {
    let Topo { order, dependents, kinds } = topo(comp, edges);
    let mut lanes: Vec<LaneOwner> = Vec::new();
    let mut rows = Vec::new();
    for n in &order {
        let mut row = Transition::arrive(&lanes, n);
        for (k, d) in dependents.get(n).into_iter().flatten().enumerate() {
            let kind = kinds.get(&(d.clone(), n.clone())).copied().unwrap_or(EdgeKind::Dep);
            row.open(Some((d.clone(), kind)), k == 0);
        }
        rows.push(row.draw(n));
        // Trailing empties are dropped so the next row's width is the lanes actually open,
        // not the widest row so far.
        lanes = row.bottom;
        while lanes.last().is_some_and(Option::is_none) {
            lanes.pop();
        }
    }
    rows
}

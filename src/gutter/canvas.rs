//! The per-row drawing surface for the dependency gutter.
//!
//! Split from the layout beside it because they are different problems: which lane an issue
//! occupies and how lanes are shortened is graph work, while this is a canvas — cells,
//! glyphs, and who owns the colour of each.

use super::{EdgeKind, LaneOwner, Row, glyph};
use std::collections::BTreeSet;

/// One row's worth of gutter, under construction.
///
/// The three arrays move together — what is drawn in a cell, who owns it, and how strongly
/// — so they travel as one thing rather than as three parallel `Vec`s threaded through a
/// closure. Horizontal bridge cells are collected and coloured after the walks: `dirs` is
/// borrowed mutably while iterating, so the owner map cannot be touched inside.
pub(super) struct Canvas {
    dirs: Vec<BTreeSet<char>>,
    owner: Vec<LaneOwner>,
    opri: Vec<i8>,
    bridges: Vec<(usize, LaneOwner)>,
    /// The node's own lane. Fixed for the row, so it is state rather than an argument
    /// every method has to be handed again.
    pos: usize,
    width: usize,
}

impl Canvas {
    pub(super) fn new(width: usize, pos: usize) -> Canvas {
        Canvas { dirs: vec![BTreeSet::new(); width], owner: vec![None; width], opri: vec![-1; width], bridges: Vec::new(), pos, width }
    }

    /// Strongest claim wins: a lane passing straight through owns its cell more firmly
    /// than a bridge crossing it.
    pub(super) fn colour(&mut self, c: usize, who: &LaneOwner, pri: i8) {
        if who.is_some() && pri > self.opri[c] {
            self.opri[c] = pri;
            self.owner[c].clone_from(who);
        }
    }

    /// Lanes that pass this row untouched, drawn before anything connects to the node.
    pub(super) fn through(&mut self, top: &[LaneOwner], arriving: &[usize]) {
        for (c, cell) in top.iter().enumerate().take(self.width) {
            if cell.is_some() && !arriving.contains(&c) && c != self.pos {
                self.dirs[c].extend(['U', 'D']);
                self.colour(c, cell, 2);
            }
        }
    }

    /// Join lane `at` to the node at `pos`, with `vert` the vertical stroke the lane leaves
    /// behind — `U` for an edge arriving from above, `D` for one starting below.
    ///
    /// The two directions were written out separately and are the same drawing: the lane
    /// points toward the node, the node points back, and every cell between them becomes a
    /// horizontal bridge. Only the vertical stroke and which row the lane is read from
    /// differ, so those are the arguments.
    pub(super) fn connect(&mut self, at: usize, lane: &LaneOwner, vert: char) {
        let pos = self.pos;
        self.dirs[at].insert(vert);
        if at == pos {
            return;
        }
        let (toward_node, toward_lane) = if at < pos { ('R', 'L') } else { ('L', 'R') };
        self.dirs[at].insert(toward_node);
        self.colour(at, lane, 2);
        self.dirs[pos].insert(toward_lane);
        self.colour(pos, lane, 1);
        for k in (at.min(pos) + 1)..at.max(pos) {
            self.dirs[k].extend(['L', 'R']);
            self.bridges.push((k, lane.clone()));
        }
    }

    /// The finished row: a glyph per lane, a bullet at the node, and the trailing blanks
    /// trimmed so a short row does not carry a tail of spaces.
    pub(super) fn render(mut self, n: &str) -> Row {
        let (pos, width) = (self.pos, self.width);
        for (k, lane) in std::mem::take(&mut self.bridges) {
            self.colour(k, &lane, 1);
        }
        let mut chars: Vec<char> = Vec::new();
        let mut owners: Vec<LaneOwner> = Vec::new();
        for c in 0..width {
            if c == pos {
                chars.push('●');
                owners.push(Some((n.to_string(), EdgeKind::Dep))); // the node's own bullet
            } else {
                chars.push(glyph(&self.dirs[c]));
                owners.push(self.owner[c].clone());
            }
            chars.push(if self.dirs[c].contains(&'R') { '─' } else { ' ' });
            owners.push(self.owner[c].clone());
        }
        while chars.last() == Some(&' ') {
            chars.pop();
            owners.pop();
        }
        (n.to_string(), chars.into_iter().collect(), owners)
    }
}

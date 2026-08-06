# How `trck deps` draws the graph

`trck deps` renders the dependency DAG as a lazygit-style gutter, topologically sorted so a
blocker always sits above what it blocks. What it *stores* is only what `trck dep --add/--remove`
put there; everything below is display. See the README for when to author an edge and at what
altitude.

## Inferred containment edges

Alongside the dependencies you authored, the graph draws an **inferred** `parent needs child`
edge for every parent/child pair — a parent is done exactly when all its children are, which
*is* a dependency. So a parent renders *below* the work it contains (it completes last), and
`trck deps <epic>` answers "what is needed to finish this epic": its open descendants plus
whatever they in turn wait on. Inferred edges are dimmed to set them apart from authored ones.

Since containment edges connect nearly the whole forest, the no-id view shows only components
holding at least one authored edge, kept whole; pure hierarchy is what `trck list` is for.

## Where an inherited edge is drawn

Dependencies are inherited downward: an edge authored on a parent binds every issue beneath
it. Where the ancestor carrying such an edge is itself on screen, it states the dependency once
and its descendants stay quiet — the containment edges already connect them, and since inheritance
reaches *every* descendant, restating it would replace one parent-level edge with a fan of `n`.
Where that ancestor is **not** on screen — `trck deps NNN --requires` on a child, say — the child
draws the inherited blocker itself, so a task blocked only through its parent never looks
actionable. `--fanout` restates it under every child regardless; the parent's own edge then
disappears as implied by its children, which is the ground truth about *which* work is blocked.
(This mirrors how the `needs #NNN (via #AAA)` row note picks its moment to speak.)

## Transitive reduction

The graph is **transitively reduced**: an edge already implied by a longer path is not drawn. If
`A` needs both `B` and `C` while `B` needs `C`, you see `A ← B ← C` and not the `A ← C` shortcut.
On a DAG that reduction is unique and preserves reachability exactly, so nothing is lost — the
path that justified dropping an edge is still on screen. It also gives parents a pleasing shape:
an epic ends up pointing only at the work nothing else waits on. Like the inferred edges this is
display-only, and it happens *after* `--omit-done` filtering, so hiding a finished middle node can
never leave its neighbours looking unrelated. A hidden edge is not a forgotten one: it stays in the
index and reappears in the graph by itself if the path that covered it ever goes away.

## Done work

The whole-graph view hides fully done components by default so completed chains don't drown out
active work; `--include-done-chains` restores them. Done nodes inside a still-active chain remain
visible as useful context, and `--omit-done` drops terminal nodes from the rendered graph without
inventing replacement edges between their neighbours.

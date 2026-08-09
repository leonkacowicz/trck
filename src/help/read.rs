//! Help for the verbs that only look
//!
//! Grouped the way `cli::dispatch` groups the same verbs, so adding one touches the
//! matching pair rather than a table nobody can find the right end of.

use super::VerbHelp;

pub(super) const VERBS: &[VerbHelp] = &[
    VerbHelp {
        verb: "show",
        tagline: "print an issue's metadata + body",
        usage: "trck show <id> [options]",
        blurb: "Print an issue's metadata, then its markdown body.",
        args: &[("id", "issue id")],
        opts: &[("--json", "one JSON document: the metadata plus a 'body' field")],
        alias_of: "",
        example: "",
    },
    VerbHelp {
        verb: "path",
        tagline: "print the absolute file path of one issue",
        usage: "trck path <id>",
        blurb: "Print the absolute path to an issue's markdown file (e.g. $(trck path 25) to open or grep it).",
        args: &[("id", "issue id")],
        opts: &[],
        alias_of: "",
        example: "",
    },
    VerbHelp {
        verb: "which",
        tagline: "resolve issue file paths back to issues",
        usage: "trck which <paths> [options]",
        blurb: "Read issue file paths (as positional args, or one per line on stdin when none are given) and print the matching issues in `list` format. The reverse of `path`/`list --paths`: pipe `rg -l PATTERN $(trck list --paths)` into it for body search. Non-issue paths are skipped.",
        args: &[("paths", "issue file paths (default: read stdin)")],
        opts: &[("--ids", "print bare issue ids (for `| xargs -n1 trck show`) instead of rows")],
        alias_of: "",
        example: "",
    },
    VerbHelp {
        verb: "list",
        tagline: "browse issues as a nested forest (filterable)",
        usage: "trck list <id> [options]",
        blurb: "Browse issues as a nested forest: every issue, children nested under their parent. Each row opens with a single-width gutter glyph: ◇ ready (an unblocked leaf nobody has started — what `trck ready` would list), ○ backlog but not ready (blocked, or an epic), ◐ in-progress or in-review, ● done. Pass an id to root the forest at one issue's subtree; --flat for a flat, globally-sorted list. Filters select the matches; their ancestor spine is kept as dimmed context. Siblings are ordered topologically — a blocker above what it blocks, the order `deps` draws — with --sort deciding between rows no dependency separates. The rank is computed over the whole graph, so a constraint routed through another epic still orders two siblings that name each other nowhere, and filtering rows out never reshuffles the ones that remain; an epic ranks at the earlier of its own row and the start of its work. A done dependency constrains nothing, so a tracker with no open dependencies comes out in plain --sort order. Parent rows show a dim points-weighted completion '% ' after the title (rolled up from leaf descendants, as in SUMMARY.md). Rows carry a dim blocking note: 'needs #NNN' for each open (non-terminal) dependency — including one inherited from an ancestor, tagged 'needs #NNN (via #AAA)' and shown only where that ancestor's own row isn't on screen — and 'blocks #NNN' for the issues waiting on this one. A note clears once the blocker is done. By default settled work is hidden: a terminal issue shows only while it is still open or sits under a non-terminal parent (so open epics keep their done children as context). Use --all to show everything; an explicit --status bypasses the prune. `tree` is an alias for this command.",
        args: &[("id", "root the forest at this issue's subtree")],
        opts: &[
            ("--flat", "flat, globally-sorted list instead of the nested forest"),
            ("--all", "include settled work (terminal issues whose parent is also terminal); by default such issues are hidden"),
            ("--status STATUS", "filter by status; comma-lists alternatives and a leading '!' negates (e.g. 'in-progress,backlog' or '!done')"),
            ("--json", "one JSON document: the nested forest, or a flat array with --flat"),
            ("--priority PRIORITY", "filter by priority"),
            ("--label LABEL", "filter to issues carrying this label"),
            ("--parent PARENT", "filter to children of this epic/parent id"),
            ("--match MATCH", "case-insensitive substring filter on the title"),
            ("--field FIELD", "filter to issues whose custom field KEY equals VALUE (repeatable; multiple are AND-ed)"),
            ("--show-field SHOW_FIELD", "append a custom field's value as a trailing column (repeatable); list is otherwise unchanged"),
            ("--sort SORT", "tie-break by created (default), id, priority, points, or field:NAME for a custom field (missing values sort last)"),
            ("--blocked", "only issues with an unmet (non-terminal) dependency"),
            ("--orphan", "only top-level issues (no parent)"),
            ("--paths", "print the absolute file path of each matching issue (flat, matches only) instead of rows — pipe into rg/grep/fzf"),
        ],
        alias_of: "",
        example: "",
    },
    VerbHelp {
        verb: "tree",
        tagline: "alias for `list`",
        usage: "trck tree <id> [options]",
        blurb: "An alias for `list`, which already nests children under their parent. Every option `list` takes works here identically.",
        args: &[("id", "root the forest at this issue's subtree")],
        opts: &[],
        alias_of: "list",
        example: "trck tree k3m",
    },
    VerbHelp {
        verb: "ready",
        tagline: "list unclaimed issues you can work on right now",
        usage: "trck ready <id> [options]",
        blurb: "List the backlog leaf issues whose dependencies are all in a terminal status — work nobody has started and nothing is blocking. A started issue (in-progress or in-review) is somebody's claim, so it is not offered here, though it still blocks its dependents and still counts toward the ranking. Ranked by demand: an issue counts for what it unblocks, so a medium task blocking an urgent one outranks a high task blocking nothing. Ties go to the number of issues blocked at that priority, then points, then id. A row ranked above its own priority is marked ↑<priority>(#id), naming what drives it. With an id, scope to that issue's subtree — what can I pick up on this epic right now. Scoping never loosens blocking: a leaf waiting on an issue outside the subtree, directly or through an edge authored on an ancestor, stays out; nor does it change the ranking, which is computed over the whole graph.",
        args: &[("id", "scope to this issue's subtree")],
        opts: &[
            ("--json", "one JSON document: \"ready\" in rank order with the demand note as fields, beside \"in_flight\""),
            ("--next", "print only the single highest-ranked ready issue"),
        ],
        alias_of: "",
        example: "",
    },
    VerbHelp {
        verb: "next",
        tagline: "print the single best issue to work on next",
        usage: "trck next <id> [options]",
        blurb: "Print only the highest-ranked ready issue (shorthand for `ready --next`) — the work that unblocks the hottest issue, not necessarily the hottest issue itself. Above it, an `in flight:` line names the leaves somebody has already started, so you can see what is taken without being offered it; with nothing started the line is omitted. With an id, the best pick within that issue's subtree, and the line scoped to it too.",
        args: &[("id", "scope to this issue's subtree")],
        opts: &[("--json", "one JSON document: the same object as `ready --json`, its \"ready\" array capped at one entry")],
        alias_of: "",
        example: "",
    },
    VerbHelp {
        verb: "deps",
        tagline: "draw the dependency DAG (lazygit-style gutter)",
        usage: "trck deps <id> [options]",
        blurb: "Draw the dependency DAG as a lazygit-style gutter, topologically sorted so a blocker sits above what it blocks. Alongside the authored depends_on edges it draws an inferred 'parent needs child' edge for each parent/child pair — a parent is done exactly when its children are — so a parent renders below the work it contains and `deps <epic>` answers what is left to finish it. A dependency authored on a parent binds every issue beneath it; a visible ancestor states it once and its descendants stay quiet, while a child shown without that ancestor draws the inherited blocker itself. --fanout restates it under every child. Inferred edges are dimmed, and are display-only: only dep --add/--remove ever changes stored dependencies. The graph is transitively reduced — an edge already implied by a longer path is not drawn (A needs B and C, B needs C: you see A <- B <- C), which is unique on a DAG and preserves reachability. With no id, every component holding at least one authored edge (pure hierarchy is what `list` is for); with an id, that issue's directed dependency line — its prerequisites and dependents. --requires/--blocks scope to one cone (prerequisites only / dependents only); --full instead widens to the issue's whole connected cluster (cousins included).",
        args: &[("id", "issue id (omit for the whole graph)")],
        opts: &[
            ("--requires", "with an id, show only its prerequisite cone (what it needs)"),
            ("--blocks", "with an id, show only its dependent cone (what waits on it)"),
            ("--json", "one JSON document {requires, blocks}; needs an issue id"),
            ("--full", "with an id, show the whole connected cluster (cousins included), not just the directed dependency line"),
            ("--include-done-chains", "in the whole graph, include components whose every issue is terminal"),
            ("--omit-done", "omit terminal issues from the rendered graph"),
            ("--fanout", "restate an inherited dependency under every child, instead of letting the visible ancestor carry it once"),
        ],
        alias_of: "",
        example: "",
    },
    VerbHelp {
        verb: "changelog",
        tagline: "list issues shipped since a date/timestamp (release notes)",
        usage: "trck changelog [options]",
        blurb: "Print, as nested markdown, the issues completed on or after the cutoff: closed in a terminal status, excluding wontfix/duplicate/superseded. Children nest under their shipped parent.",
        args: &[],
        opts: &[("--since SINCE", "cutoff (inclusive): a date (2026-06-10) or timestamp (2026-06-10T14:00:00Z)")],
        alias_of: "",
        example: "",
    },
    VerbHelp {
        verb: "diff",
        tagline: "show what changed in the tracker between two points",
        usage: "trck diff [rev|rev1..rev2] [options]",
        blurb: "Compare the tracker at two points and report what changed -- statuses, priorities, parents, labels, dependencies -- in the tracker's own vocabulary, rather than as raw index.jsonl text. With no arguments, HEAD vs the working tree. Sources are VCS-agnostic underneath: --from/--to accept any index.jsonl file, a whole tracker dir (bodies included), or '-' for stdin, and never invoke git.",
        args: &[("rev", "a git revision, or `a..b` for a range; omitted, the older side is HEAD")],
        opts: &[
            ("--from FROM", "the older side as a source rather than a revision: an index.jsonl, a tracker dir, or `-` for stdin"),
            ("--to TO", "the newer side, same forms (default: the working tree)"),
        ],
        alias_of: "",
        example: "trck diff                               # HEAD vs the working tree
  trck diff main                          # main vs the working tree
  trck diff v0.22..v0.23                  # between two tags
  trck diff --from old-index.jsonl        # no git involved
  trck diff --from a.jsonl --to b.jsonl   # two explicit sides
  git show main:issues/index.jsonl | trck diff --from -",
    },
];

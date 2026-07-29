from __future__ import annotations
import argparse
from .cmd_maint import cmd_changelog, cmd_check, cmd_done, cmd_init, cmd_install_hook, cmd_normalize, cmd_renumber, cmd_review, cmd_start, cmd_summary, cmd_version
from .cmd_mutate import cmd_dep, cmd_label, cmd_mv, cmd_new, cmd_set
from .cmd_query import cmd_deps, cmd_list, cmd_next, cmd_path, cmd_ready, cmd_show, cmd_which
from .cmd_selfmgmt import cmd_update

TOP_EPILOG = """\
MODEL
  Each issue is a markdown file under the tracker dir plus one line in
  index.jsonl, keyed by a short random alphanumeric id (e.g. k3m9x2a). index.jsonl and
  SUMMARY.md are GENERATED -- never hand-edit them, and never move or rename
  issue files by hand; the verbs do that. The only thing you edit by hand is
  an issue's prose body (Summary / Acceptance criteria / Notes).

  ids         short random alphanumeric strings (7 chars from a base32 alphabet
              with look-alike chars removed). Any unambiguous prefix works in
              place of a full id: `trck show k3m` matches k3m9x2a. An ambiguous
              prefix is an error that lists the candidates. Legacy integer-id
              trackers keep working; `trck renumber` migrates them.
  status      every issue has exactly one; move it with mv / start / review /
              done. A status may set "actionable": false (as in-review does) --
              work waiting there stays out of ready/next, but still blocks.
  metadata    priority, points, parent, spec, pr, kind, title, slug -- change
              with `trck set` (NOT by editing index.jsonl). labels -- a flat
              set of free-text tags; change with `trck label`.
  hierarchy   --parent / --kind epic build an epic tree (containment).
  deps        dep / --depends build a blocking graph (ordering). A parent is
              "part of"; a dep is "must come first" -- they are independent.
              Deps climb the tree: depending on a parent depends on its whole
              subtree; a parent's deps are inherited by its children. A node
              and its own ancestor/descendant can't depend on each other.
  points      a leaf's weight; rolls up to its epic for progress totals.
  config      the status vocabulary, aliases, priorities, kinds, and
              resolutions all come from trck.json (defaults: backlog ->
              ongoing -> in-review -> done; start=ongoing, review=in-review,
              done=done).

RECOMMENDED USAGE
  parent      decomposition, not categorization: make B a child of A only when
              the children break A down into sub-tasks -- A *is* the sum of its
              children. Litmus test: A can be marked done exactly when all its
              children are. A generic bucket of similar tasks is a LABEL, not a
              parent. Children are unordered; to sequence sub-tasks, chain them
              with dependencies, not by nesting.
  dependency  hard ordering (MUST): `A depends on B` means B must be done before
              A -- B blocks A; ready/next hide a task until its deps are met.
              Climbs the hierarchy: a parent's dep binds every child; depending
              on a parent waits for its whole subtree. Put the arrow at the right
              altitude (parent vs. specific children) and be precise.
  priority    soft ordering (SHOULD): a preference for what to pick up next, not
              a constraint -- nothing is blocked. Set it on the issue that
              carries the urgency; ready/next propagate it back along the
              dependency and parent edges, so a blocker inherits the urgency of
              whatever is waiting on it (marked `↑<priority>(#id)` on the row).

TYPICAL FLOW
  trck new "Add CSV export" --priority high     # create; prints the new id
  trck show 7                                    # then hand-edit its body prose
  trck set 7 --points 3 --parent 4              # adjust metadata anytime
  trck set 7 --field assignee=leon --field component=ui  # arbitrary metadata
  trck start 7                                   # -> ongoing
  trck review 7 https://github.com/o/r/pull/12  # -> in-review, links the PR
  trck done 7 --resolution superseded            # -> done (resolution optional)
  trck ready         /  trck next                # unblocked work, ranked by demand
  trck list                                      # active forest (settled subtrees hidden)
  trck list --all                                # include settled (done) work too
  trck list --status ongoing                     # what's in flight
  trck list --status '!done' --sort priority     # open work, hottest first
  trck list --match parser --orphan              # title search, top-level only
  trck list --field assignee=leon --sort field:component # filter + sort custom fields
  trck list --status '!done' --show-field assignee       # add a custom-field column
  trck tree 4                                    # hierarchy view
  trck deps          /  trck deps 7              # dependency DAG (whole / one issue's line)
  trck check                                     # MUST pass before you commit

DISCOVERY
  Finds the tracker by walking up for a dir containing trck.json; override with
  --dir or $TRCK_DIR. Run `trck <verb> -h` for per-verb options.
"""


def build_parser() -> argparse.ArgumentParser:
    raw = argparse.RawDescriptionHelpFormatter
    p = argparse.ArgumentParser(
        prog="trck", description="deterministic in-repo issue tracker",
        epilog=TOP_EPILOG, formatter_class=raw)
    p.add_argument("--dir", help="tracker dir (overrides discovery and $TRCK_DIR)")
    sub = p.add_subparsers(dest="cmd", required=True)

    n = sub.add_parser(
        "new", help="create a new issue (then edit its prose)", formatter_class=raw,
        description="Create an issue: write its markdown file from a prose "
                    "template and add it to the index; prints the new path.",
        epilog='examples:\n  trck new "Add CSV export" --priority high '
               '--parent 4 --depends 5,6')
    n.add_argument("title", help="short title (also derives the slug)")
    n.add_argument("--priority", help="configured priority (default: first in trck.json)")
    n.add_argument("--points", type=int, help="leaf weight for rollups (default 1)")
    n.add_argument("--kind", help="configured kind (default: first in trck.json, e.g. task)")
    n.add_argument("--parent", help="id of the epic to nest this under")
    n.add_argument("--depends", help="comma-separated ids this issue depends on (must be done first)")
    n.add_argument("--spec", help="path to a spec/design doc")
    n.add_argument("--pr", help="pull-request URL (absolute http(s) link)")
    n.add_argument("--slug", help="override the auto-derived filename slug")
    n.set_defaults(func=cmd_new, priority=None)

    mv = sub.add_parser("mv", help="move an issue to a status (vocabulary-agnostic)",
                        description="Move an issue to any configured status (vocabulary-agnostic).")
    mv.add_argument("id", help="issue id")
    mv.add_argument("status", help="target status (must be configured)")
    mv.add_argument("--resolution",
                    help="resolution label; only valid when moving to a terminal status")
    mv.add_argument("--pr", help="record a pull-request URL as part of the move")
    mv.set_defaults(func=cmd_mv)

    st = sub.add_parser("start", help="alias: move to the configured 'start' status",
                        description="Alias: move an issue to the status configured as the 'start' alias.")
    st.add_argument("id", help="issue id")
    st.set_defaults(func=cmd_start)

    rv = sub.add_parser(
        "review", help="alias: move to the configured 'review' status (and link a PR)",
        formatter_class=raw,
        description="Alias: move an issue to the status configured as the 'review' "
                    "alias, and — given a URL — record it as the issue's pull request "
                    "in one step. An issue in a non-actionable status like in-review "
                    "stays out of ready/next, but still blocks whatever depends on it "
                    "until the PR lands.",
        epilog="examples:\n  trck review 7 https://github.com/o/r/pull/12")
    rv.add_argument("id", help="issue id")
    rv.add_argument("url", nargs="?", help="pull-request URL to link (optional)")
    rv.set_defaults(func=cmd_review)

    dn = sub.add_parser("done", help="alias: move to the configured 'done' status",
                        description="Alias: move an issue to the status configured as the 'done' alias.")
    dn.add_argument("id", help="issue id")
    dn.add_argument("--resolution", help="resolution label (from the configured set)")
    dn.set_defaults(func=cmd_done)

    se = sub.add_parser(
        "set", help="edit metadata fields",
        description="Edit an issue's metadata in place. Pass 'none' to clear "
                    "parent/spec. Changing --slug renames the file; changing "
                    "--title also rewrites the body's H1.")
    se.add_argument("id", help="issue id")
    se.add_argument("--priority", help="configured priority")
    se.add_argument("--points", type=int, help="leaf weight (error if the issue has children)")
    se.add_argument("--parent", help="epic id, or 'none' to clear")
    se.add_argument("--spec", help="path, or 'none' to clear")
    se.add_argument("--pr", help="pull-request URL, or 'none' to clear")
    se.add_argument("--kind", help="configured kind")
    se.add_argument("--title", help="new title (also rewrites the body's H1)")
    se.add_argument("--slug", help="override the filename slug (renames the file)")
    se.add_argument("--field", action="append", metavar="KEY=VALUE",
                    help="set a custom field (repeatable); empty value clears it")
    se.add_argument("--unset", action="append", metavar="KEY",
                    help="remove a custom field (repeatable)")
    se.add_argument("--auto", action="store_true",
                    help="clear a manual status override; let status derive from children")
    se.set_defaults(func=cmd_set)

    dp = sub.add_parser(
        "dep", help="add/remove a dependency edge", formatter_class=raw,
        description="Add or remove a dependency edge: make <id> depend on "
                    "another issue (which must be done first).",
        epilog="examples:\n  trck dep 7 --add 5    # 7 now waits on 5")
    dp.add_argument("id", help="issue id")
    dp.add_argument("--add", help="id this issue should depend on")
    dp.add_argument("--remove", help="id to remove from this issue's dependencies")
    dp.set_defaults(func=cmd_dep)

    lb = sub.add_parser(
        "label", help="add/remove labels on an issue", formatter_class=raw,
        description="Add or remove free-text labels on an issue. Labels are a "
                    "flat, unordered set; both flags are repeatable.",
        epilog="examples:\n  trck label 7 --add backend --add urgent --remove stale")
    lb.add_argument("id", help="issue id")
    lb.add_argument("--add", action="append", metavar="LABEL",
                    help="label to add (repeatable)")
    lb.add_argument("--remove", action="append", metavar="LABEL",
                    help="label to remove (repeatable)")
    lb.set_defaults(func=cmd_label)

    sh = sub.add_parser("show", help="print an issue's metadata + body",
                        description="Print an issue's metadata, then its markdown body.")
    sh.add_argument("id", help="issue id")
    sh.add_argument("--json", action="store_true", help="raw JSON metadata (machine-readable)")
    sh.set_defaults(func=cmd_show)

    pa = sub.add_parser("path", help="print the absolute file path of one issue",
                        description="Print the absolute path to an issue's markdown "
                                    "file (e.g. $(trck path 25) to open or grep it).")
    pa.add_argument("id", help="issue id")
    pa.set_defaults(func=cmd_path)

    wh = sub.add_parser(
        "which", help="resolve issue file paths back to issues",
        description="Read issue file paths (as positional args, or one per line on "
                    "stdin when none are given) and print the matching issues in "
                    "`list` format. The reverse of `path`/`list --paths`: pipe "
                    "`rg -l PATTERN $(trck list --paths)` into it for body search. "
                    "Non-issue paths are skipped.")
    wh.add_argument("paths", nargs="*", help="issue file paths (default: read stdin)")
    wh.add_argument("--ids", action="store_true",
                    help="print bare issue ids (for `| xargs -n1 trck show`) instead of rows")
    wh.set_defaults(func=cmd_which)

    ls = sub.add_parser("list", aliases=["tree"], help="browse issues as a nested forest (filterable)",
                        description="Browse issues as a nested forest: every issue, children nested "
                                    "under their parent. Pass an id to root the forest at one issue's "
                                    "subtree; --flat for a flat, globally-sorted list. Filters select "
                                    "the matches; their ancestor spine is kept as dimmed context. "
                                    "Parent rows show a dim points-weighted completion '% ' after the "
                                    "title (rolled up from leaf descendants, as in SUMMARY.md). "
                                    "Rows carry a dim blocking note: 'needs #NNN' for each open "
                                    "(non-terminal) dependency — including one inherited from an "
                                    "ancestor, tagged 'needs #NNN (via #AAA)' and shown only where "
                                    "that ancestor's own row isn't on screen — and 'blocks #NNN' for "
                                    "the issues waiting on this one. A note clears once the blocker "
                                    "is done. "
                                    "By default settled work is hidden: a terminal issue shows only "
                                    "while it is still open or sits under a non-terminal parent (so "
                                    "open epics keep their done children as context). Use --all to "
                                    "show everything; an explicit --status bypasses the prune. "
                                    "`tree` is an alias for this command.")
    ls.add_argument("id", nargs="?", help="root the forest at this issue's subtree")
    ls.add_argument("--flat", action="store_true",
                    help="flat, globally-sorted list instead of the nested forest")
    ls.add_argument("--all", action="store_true",
                    help="include settled work (terminal issues whose parent is also "
                         "terminal); by default such issues are hidden")
    ls.add_argument("--status",
                    help="filter by status; comma-lists alternatives and a leading "
                         "'!' negates (e.g. 'ongoing,backlog' or '!done')")
    ls.add_argument("--kind", help="filter by kind")
    ls.add_argument("--priority", help="filter by priority")
    ls.add_argument("--label", help="filter to issues carrying this label")
    ls.add_argument("--parent", help="filter to children of this epic/parent id")
    ls.add_argument("--match", help="case-insensitive substring filter on the title")
    ls.add_argument("--field", action="append", metavar="KEY=VALUE",
                    help="filter to issues whose custom field KEY equals VALUE "
                         "(repeatable; multiple are AND-ed)")
    ls.add_argument("--show-field", action="append", metavar="NAME", dest="show_field",
                    help="append a custom field's value as a trailing column "
                         "(repeatable); list is otherwise unchanged")
    ls.add_argument("--sort", metavar="FIELD",
                    help="order by created (default), id, priority, points, or "
                         "field:NAME for a custom field (missing values sort last)")
    ls.add_argument("--blocked", action="store_true",
                    help="only issues with an unmet (non-terminal) dependency")
    ls.add_argument("--orphan", "--no-parent", dest="orphan", action="store_true",
                    help="only top-level issues (no parent)")
    ls.add_argument("--paths", action="store_true",
                    help="print the absolute file path of each matching issue "
                         "(flat, matches only) instead of rows — pipe into rg/grep/fzf")
    ls.set_defaults(func=cmd_list)

    rd = sub.add_parser(
        "ready", help="list issues you can work on right now",
        description="List not-done leaf issues whose dependencies are all in a "
                    "terminal status, ranked by demand: an issue counts for what it "
                    "unblocks, so a medium task blocking an urgent one outranks a high "
                    "task blocking nothing. Ties go to the number of issues blocked at "
                    "that priority, then points, then id. A row ranked above its own "
                    "priority is marked ↑<priority>(#id), naming what drives it. With "
                    "an id, scope to that issue's subtree — what can I pick up on this "
                    "epic right now. Scoping never loosens blocking: a leaf waiting on "
                    "an issue outside the subtree, directly or through an edge "
                    "authored on an ancestor, stays out; nor does it change the "
                    "ranking, which is computed over the whole graph.")
    rd.add_argument("id", nargs="?", help="scope to this issue's subtree")
    rd.add_argument("--next", action="store_true",
                    help="print only the single highest-ranked ready issue")
    rd.set_defaults(func=cmd_ready)

    nx = sub.add_parser("next", help="print the single best issue to work on next",
                        description="Print only the highest-ranked ready issue "
                                    "(shorthand for `ready --next`) — the work that "
                                    "unblocks the hottest issue, not necessarily the "
                                    "hottest issue itself. With an id, the best pick "
                                    "within that issue's subtree.")
    nx.add_argument("id", nargs="?", help="scope to this issue's subtree")
    nx.set_defaults(func=cmd_next)

    dz = sub.add_parser(
        "deps", help="draw the dependency DAG (lazygit-style gutter)",
        description="Draw the dependency DAG as a lazygit-style gutter, topologically "
                    "sorted so a blocker sits above what it blocks. Alongside the "
                    "authored depends_on edges it draws an inferred 'parent needs "
                    "child' edge for each parent/child pair — a parent is done exactly "
                    "when its children are — so a parent renders below the work it "
                    "contains and `deps <epic>` answers what is left to finish it. "
                    "A dependency authored on a parent binds every issue beneath it; "
                    "a visible ancestor states it once and its descendants stay quiet, "
                    "while a child shown without that ancestor draws the inherited "
                    "blocker itself. --fanout restates it under every child. "
                    "Inferred edges are dimmed, and are display-only: only dep "
                    "--add/--remove ever changes stored dependencies. The graph is "
                    "transitively reduced — an edge already implied by a longer path "
                    "is not drawn (A needs B and C, B needs C: you see A <- B <- C), "
                    "which is unique on a DAG and preserves reachability. With no id, every "
                    "component holding at least one authored edge (pure hierarchy is "
                    "what `list` is for); with an id, that issue's directed dependency "
                    "line — its prerequisites and dependents. --requires/--blocks scope "
                    "to one cone (prerequisites only / dependents only); --full instead "
                    "widens to the issue's whole connected cluster (cousins included).")
    dz.add_argument("id", nargs="?", help="issue id (omit for the whole graph)")
    dz.add_argument("--requires", action="store_true",
                    help="with an id, show only its prerequisite cone (what it needs)")
    dz.add_argument("--blocks", action="store_true",
                    help="with an id, show only its dependent cone (what waits on it)")
    dz.add_argument("--full", action="store_true",
                    help="with an id, show the whole connected cluster (cousins "
                         "included), not just the directed dependency line")
    dz.add_argument("--include-done-chains", action="store_true",
                    help="in the whole graph, include components whose every issue is terminal")
    dz.add_argument("--omit-done", action="store_true",
                    help="omit terminal issues from the rendered graph")
    dz.add_argument("--fanout", action="store_true",
                    help="restate an inherited dependency under every child, instead "
                         "of letting the visible ancestor carry it once")
    dz.add_argument("--graph", action="store_true", help=argparse.SUPPRESS)  # now default; kept as a no-op
    dz.set_defaults(func=cmd_deps)

    cl = sub.add_parser("changelog",
                        help="list issues shipped since a date/timestamp (release notes)",
                        description="Print, as nested markdown, the issues completed "
                                    "on or after the cutoff: closed in a terminal status, "
                                    "excluding wontfix/duplicate/superseded. Children "
                                    "nest under their shipped parent.")
    cl.add_argument("--since", required=True, metavar="DATE|TIMESTAMP",
                    help="cutoff (inclusive): a date (2026-06-10) or timestamp (2026-06-10T14:00:00Z)")
    cl.set_defaults(func=cmd_changelog)

    ck = sub.add_parser("check", help="validate consistency (nonzero exit on error)",
                        description="Validate index/file/graph consistency; nonzero "
                                    "exit on any error. Run before committing.")
    ck.set_defaults(func=cmd_check)

    su = sub.add_parser("summary", help="regenerate SUMMARY.md",
                        description="Regenerate SUMMARY.md from the index.")
    su.set_defaults(func=cmd_summary)

    nm = sub.add_parser("normalize", help="rewrite index.jsonl in canonical slim form",
                        description="Rewrite index.jsonl in canonical slim form "
                                    "(stable key order, stripped defaults).")
    nm.set_defaults(func=cmd_normalize)

    rn = sub.add_parser("renumber",
                        help="convert legacy integer ids to random alphanumeric ids",
                        description="One-shot migration: replace every legacy integer "
                                    "id with a random alphanumeric id, rewriting "
                                    "parent/depends_on, recording the prior id in "
                                    "legacy_id (a resolvable alias), and renaming files. "
                                    "Idempotent; random ids are left untouched.")
    rn.set_defaults(func=cmd_renumber)

    ih = sub.add_parser("install-hook", help="install the pre-commit consistency hook",
                        description="Install a git pre-commit hook that runs `trck check`.")
    ih.set_defaults(func=cmd_install_hook)

    iv = sub.add_parser("init", help="scaffold a tracker into the current repo",
                        description="Scaffold a tracker (trck.json + dirs) into the "
                                    "current repo; vendors the engine by default.")
    iv.add_argument("target", nargs="?", default=None, help="tracker dir to create (default: issues)")
    iv.add_argument("--dir", dest="init_dir", default=None, help="same as the positional dir")
    iv.add_argument("--no-vendor", action="store_true", help="don't copy the engine into the tracker dir")
    iv.add_argument("--hook", action="store_true", help="also install the pre-commit hook")
    iv.add_argument("--force", action="store_true", help="overwrite existing tracker files")
    iv.set_defaults(func=cmd_init)

    up = sub.add_parser("update", help="self-update from the canonical repo",
                        description="Self-update the engine from the canonical repo's latest release.")
    up.add_argument("--check", action="store_true",
                    help="report whether an update is available without applying it")
    up.add_argument("--ref", help="update to a specific git ref/tag instead of the latest release")
    up.set_defaults(func=cmd_update)

    ve = sub.add_parser("version", help="print the running trck version",
                        description="Print the running trck version.")
    ve.set_defaults(func=cmd_version)

    return p


def main() -> None:
    args = build_parser().parse_args()
    args.func(args)


if __name__ == "__main__":
    main()

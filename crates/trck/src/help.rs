//! Per-verb help.
//!
//! `trck <verb> --help` has to answer about the verb, not the program. It used to print the
//! same summary whatever came before it, which is the least useful moment to be told what
//! trck is in general.
//!
//! The text is inherited rather than invented: it comes from the argparse definitions the
//! engine's predecessor carried, where every option already had a sentence written for it.
//! Rewriting them from scratch would have quietly changed what the tool claims to do.
//!
//! What keeps it honest is the test at the bottom: every flag a verb accepts must be
//! documented here, and every flag documented here must be one the verb accepts. Help that
//! drifts from the parser is worse than no help, because it is believed.

/// One verb's help: what it is for, how to call it, and what each part means.
struct VerbHelp {
    verb: &'static str,
    /// One line, as the verb list would describe it.
    tagline: &'static str,
    usage: &'static str,
    /// A paragraph, wrapped when rendered rather than pre-wrapped here.
    blurb: &'static str,
    /// Positional arguments — or, for `repo`, its subcommands.
    args: &'static [(&'static str, &'static str)],
    opts: &'static [(&'static str, &'static str)],
    /// Empty when there is nothing worth showing.
    example: &'static str,
    /// Set when this verb is another one under a second name. Its options are that verb's,
    /// so they are documented once, there, and pointed at from here — repeating fifteen
    /// filter flags in two places is how the two copies start disagreeing.
    alias_of: &'static str,
}

/// `--dir` is accepted by every verb and explained once, in the global help, rather than
/// repeated two dozen times in a list of options that are actually about the verb.
const GLOBAL_OPTS: &[(&str, &str)] = &[("--dir DIR", "the tracker to act on, overriding discovery and $TRCK_DIR")];

const HELP: &[VerbHelp] = &[
    VerbHelp {
        verb: "new",
        tagline: "create a new issue (then edit its prose)",
        usage: "trck new <title> [options]",
        blurb: "Create an issue: write its markdown file from a prose template and add it to the index; prints the new path.",
        args: &[("title", "short title (also derives the slug)")],
        opts: &[
            ("--priority PRIORITY", "urgent, high, medium, low or lowest (default: medium)"),
            ("--points POINTS", "leaf weight for rollups (default 1)"),
            ("--parent PARENT", "id of the epic to nest this under"),
            ("--requires REQUIRES", "comma-separated ids this issue depends on (must be done first)"),
            ("--spec SPEC", "path to a spec/design doc"),
            ("--review-url REVIEW_URL", "where the output will be reviewed (absolute http(s) URL)"),
            ("--slug SLUG", "override the auto-derived filename slug"),
            ("--id ID", "use this id instead of generating one (for importing issues from another tracker, or restoring one)"),
        ],
        alias_of: "",
        example: "trck new \"Add CSV export\" --priority high --parent 4 --requires 5,6",
    },
    VerbHelp {
        verb: "mv",
        tagline: "move an issue to a status",
        usage: "trck mv <id> <status> [options]",
        blurb: "Move an issue to any of the four statuses.",
        args: &[("id", "issue id"), ("status", "backlog, ongoing, in-review or done")],
        opts: &[
            ("--resolution RESOLUTION", "why it closed without shipping (superseded, wontfix, duplicate); only valid when moving to a terminal status"),
            ("--review-url REVIEW_URL", "record a review URL as part of the move"),
        ],
        alias_of: "",
        example: "",
    },
    VerbHelp {
        verb: "start",
        tagline: "alias: move to ongoing",
        usage: "trck start <id>",
        blurb: "Alias: move an issue to ongoing.",
        args: &[("id", "issue id")],
        opts: &[],
        alias_of: "",
        example: "",
    },
    VerbHelp {
        verb: "review",
        tagline: "alias: move to in-review (and link a PR)",
        usage: "trck review <id> [url]",
        blurb: "Alias: move an issue to in-review and — given a URL — record it as the issue's pull request, in one step. An issue there stays out of ready/next (nothing to pick up), but still blocks whatever depends on it until the PR lands.",
        args: &[("id", "issue id"), ("url", "pull-request URL to link (optional)")],
        opts: &[("--review-url REVIEW_URL", "the same URL as a flag, for a caller that would rather be explicit")],
        alias_of: "",
        example: "trck review 7 https://github.com/o/r/pull/12",
    },
    VerbHelp {
        verb: "done",
        tagline: "alias: move to done",
        usage: "trck done <id> [options]",
        blurb: "Alias: move an issue to done.",
        args: &[("id", "issue id")],
        opts: &[("--resolution RESOLUTION", "why it closed without shipping (superseded, wontfix, duplicate); omit it when the work actually shipped")],
        alias_of: "",
        example: "",
    },
    VerbHelp {
        verb: "set",
        tagline: "edit metadata fields",
        usage: "trck set <id> [options]",
        blurb: "Edit an issue's metadata in place. Pass 'none' to clear parent/spec. Changing --slug renames the file; changing --title also rewrites the body's H1.",
        args: &[("id", "issue id")],
        opts: &[
            ("--priority PRIORITY", "urgent, high, medium, low or lowest"),
            ("--points POINTS", "leaf weight (error if the issue has children)"),
            ("--parent PARENT", "epic id, or 'none' to clear"),
            ("--spec SPEC", "path, or 'none' to clear"),
            ("--review-url REVIEW_URL", "review URL, or 'none' to clear"),
            ("--title TITLE", "new title (also rewrites the body's H1)"),
            ("--slug SLUG", "override the filename slug (renames the file)"),
            ("--field FIELD", "set a custom field (repeatable); empty value clears it"),
            ("--unset UNSET", "remove a custom field (repeatable)"),
            ("--auto", "clear a manual status override; let status derive from children"),
        ],
        alias_of: "",
        example: "",
    },
    VerbHelp {
        verb: "dep",
        tagline: "add/remove a dependency edge",
        usage: "trck dep <id> [options]",
        blurb: "Add or remove a dependency edge: make <id> depend on another issue (which must be done first).",
        args: &[("id", "issue id")],
        opts: &[("--add ADD", "id this issue should depend on"), ("--remove REMOVE", "id to remove from this issue's dependencies")],
        alias_of: "",
        example: "trck dep 7 --add 5    # 7 now waits on 5",
    },
    VerbHelp {
        verb: "label",
        tagline: "add/remove labels on an issue",
        usage: "trck label <id> [options]",
        blurb: "Add or remove free-text labels on an issue. Labels are a flat, unordered set; both flags are repeatable.",
        args: &[("id", "issue id")],
        opts: &[("--add ADD", "label to add (repeatable)"), ("--remove REMOVE", "label to remove (repeatable)")],
        alias_of: "",
        example: "trck label 7 --add backend --add urgent --remove stale",
    },
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
        blurb: "Browse issues as a nested forest: every issue, children nested under their parent. Pass an id to root the forest at one issue's subtree; --flat for a flat, globally-sorted list. Filters select the matches; their ancestor spine is kept as dimmed context. Parent rows show a dim points-weighted completion '% ' after the title (rolled up from leaf descendants, as in SUMMARY.md). Rows carry a dim blocking note: 'needs #NNN' for each open (non-terminal) dependency — including one inherited from an ancestor, tagged 'needs #NNN (via #AAA)' and shown only where that ancestor's own row isn't on screen — and 'blocks #NNN' for the issues waiting on this one. A note clears once the blocker is done. By default settled work is hidden: a terminal issue shows only while it is still open or sits under a non-terminal parent (so open epics keep their done children as context). Use --all to show everything; an explicit --status bypasses the prune. `tree` is an alias for this command.",
        args: &[("id", "root the forest at this issue's subtree")],
        opts: &[
            ("--flat", "flat, globally-sorted list instead of the nested forest"),
            ("--all", "include settled work (terminal issues whose parent is also terminal); by default such issues are hidden"),
            ("--status STATUS", "filter by status; comma-lists alternatives and a leading '!' negates (e.g. 'ongoing,backlog' or '!done')"),
            ("--json", "one JSON document: the nested forest, or a flat array with --flat"),
            ("--priority PRIORITY", "filter by priority"),
            ("--label LABEL", "filter to issues carrying this label"),
            ("--parent PARENT", "filter to children of this epic/parent id"),
            ("--match MATCH", "case-insensitive substring filter on the title"),
            ("--field FIELD", "filter to issues whose custom field KEY equals VALUE (repeatable; multiple are AND-ed)"),
            ("--show-field SHOW_FIELD", "append a custom field's value as a trailing column (repeatable); list is otherwise unchanged"),
            ("--sort SORT", "order by created (default), id, priority, points, or field:NAME for a custom field (missing values sort last)"),
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
        tagline: "list issues you can work on right now",
        usage: "trck ready <id> [options]",
        blurb: "List not-done leaf issues whose dependencies are all in a terminal status, ranked by demand: an issue counts for what it unblocks, so a medium task blocking an urgent one outranks a high task blocking nothing. Ties go to the number of issues blocked at that priority, then points, then id. A row ranked above its own priority is marked ↑<priority>(#id), naming what drives it. With an id, scope to that issue's subtree — what can I pick up on this epic right now. Scoping never loosens blocking: a leaf waiting on an issue outside the subtree, directly or through an edge authored on an ancestor, stays out; nor does it change the ranking, which is computed over the whole graph.",
        args: &[("id", "scope to this issue's subtree")],
        opts: &[
            ("--json", "one JSON document: an array in rank order, with the demand note as fields"),
            ("--next", "print only the single highest-ranked ready issue"),
        ],
        alias_of: "",
        example: "",
    },
    VerbHelp {
        verb: "next",
        tagline: "print the single best issue to work on next",
        usage: "trck next <id> [options]",
        blurb: "Print only the highest-ranked ready issue (shorthand for `ready --next`) — the work that unblocks the hottest issue, not necessarily the hottest issue itself. With an id, the best pick within that issue's subtree.",
        args: &[("id", "scope to this issue's subtree")],
        opts: &[("--json", "one JSON document: the same array as `ready --json`, capped at one entry")],
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
    VerbHelp {
        verb: "check",
        tagline: "validate consistency (nonzero exit on error)",
        usage: "trck check",
        blurb: "Validate index/file/graph consistency; nonzero exit on any error. Run before committing.",
        args: &[],
        opts: &[],
        alias_of: "",
        example: "",
    },
    VerbHelp {
        verb: "summary",
        tagline: "regenerate SUMMARY.md",
        usage: "trck summary",
        blurb: "Regenerate SUMMARY.md from the index.",
        args: &[],
        opts: &[],
        alias_of: "",
        example: "",
    },
    VerbHelp {
        verb: "html",
        tagline: "write a self-contained HTML view of the tracker",
        usage: "trck html [options]",
        blurb: "Render the whole tracker as one self-contained HTML file \u{2014} browsable, filterable, and referencing nothing external, so it opens from disk and survives being emailed around.",
        args: &[],
        opts: &[("--out OUT", "where to write it (default: <tracker>/issues.html)"), ("--cmd CMD", "label the page with the command that produced it")],
        alias_of: "",
        example: "trck html --out docs/issues.html",
    },
    VerbHelp {
        verb: "repo",
        tagline: "tracker maintenance (normalize, install-hook, …)",
        usage: "trck repo <subcommand> [options]",
        blurb: "Maintenance verbs that operate on an existing tracker. Run them rarely; the daily verbs live at the top level.",
        args: &[
            ("normalize", "rewrite index.jsonl in canonical slim form (no data change)"),
            ("install-hook", "install the pre-commit consistency hook in this clone"),
            ("setup-git", "declare trck's merge drivers and register them in this clone"),
            ("migrate-layout", "move issue bodies out of per-status folders into items/"),
            ("merge-index", "the index.jsonl merge driver \u{2014} git invokes it, you do not"),
            ("merge-summary", "the SUMMARY.md merge driver \u{2014} git invokes it, you do not"),
        ],
        opts: &[("--dry-run", "migrate-layout only: show the moves, write nothing")],
        alias_of: "",
        example: "trck repo setup-git",
    },
    VerbHelp {
        verb: "init",
        tagline: "scaffold a tracker into the current repo",
        usage: "trck init <target> [options]",
        blurb: "Scaffold a tracker into the current repo: trck.json, and the docs that explain how to drive it. No engine is written \u{2014} trck is installed on the machine, never committed to the repository it serves.",
        args: &[("dir", "tracker dir to create (default: issues)")],
        opts: &[
            ("--dir DIR", "same as the positional dir; giving both is an error"),
            ("--hook", "also install the pre-commit consistency hook"),
            ("--force", "overwrite the config and scaffolded docs of an existing tracker"),
            ("--no-vendor", "accepted and ignored: nothing is ever vendored"),
        ],
        alias_of: "",
        example: "trck init issues --hook",
    },
    VerbHelp {
        verb: "version",
        tagline: "print the running trck version",
        usage: "trck version",
        blurb: "Print the running trck version.",
        args: &[],
        opts: &[],
        alias_of: "",
        example: "",
    },
];

/// Wrap `text` to `width` columns, prefixing every line with `indent`.
///
/// Written out rather than pre-wrapped in the table because the table is edited far more
/// often than this is, and a paragraph stored as one string is one a person can rewrite
/// without re-flowing it by hand.
fn wrap(text: &str, width: usize, indent: &str) -> String {
    let mut lines: Vec<String> = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        if !line.is_empty() && line.chars().count() + 1 + word.chars().count() > width {
            lines.push(std::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    if !line.is_empty() {
        lines.push(line);
    }
    let mut out = String::new();
    for l in &lines {
        out.push_str(indent);
        out.push_str(l);
        out.push('\n');
    }
    out
}

/// A two-column block: the term, then its description wrapped in the remaining width.
fn columns(rows: &[(&str, &str)], width: usize) -> String {
    let gutter = rows.iter().map(|(t, _)| t.chars().count()).max().unwrap_or(0) + 2;
    let mut out = String::new();
    for (term, desc) in rows {
        let indent = " ".repeat(gutter + 2);
        let body = wrap(desc, width.saturating_sub(gutter + 2), &indent);
        out.push_str("  ");
        out.push_str(term);
        out.push_str(&" ".repeat(gutter.saturating_sub(term.chars().count())));
        out.push_str(body.trim_start());
    }
    out
}

/// Help for one verb, or `None` when it has none — in which case the caller falls back to
/// the program's own help rather than saying nothing.
pub(crate) fn for_verb(verb: &str) -> Option<String> {
    let h = HELP.iter().find(|h| h.verb == verb)?;
    let width = 88;
    let mut out = format!("trck {} — {}\n\n", h.verb, h.tagline);
    out.push_str("usage: ");
    out.push_str(h.usage);
    out.push_str("\n\n");
    out.push_str(&wrap(h.blurb, width, ""));
    if !h.args.is_empty() {
        let label = if h.verb == "repo" { "\nsubcommands:\n" } else { "\narguments:\n" };
        out.push_str(label);
        out.push_str(&columns(h.args, width));
    }
    if !h.opts.is_empty() {
        out.push_str("\noptions:\n");
        out.push_str(&columns(h.opts, width));
    }
    if !h.alias_of.is_empty() {
        out.push_str("\noptions: every option `");
        out.push_str(h.alias_of);
        out.push_str("` takes — see `trck ");
        out.push_str(h.alias_of);
        out.push_str(" --help`.\n");
    }
    out.push_str("\nglobal:\n");
    out.push_str(&columns(GLOBAL_OPTS, width));
    if !h.example.is_empty() {
        out.push_str("\nexample:\n");
        for line in h.example.lines() {
            out.push_str("  ");
            out.push_str(line.trim());
            out.push('\n');
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    // Tests assert; that is their job. The crate denies unwrap/expect/panic because a
    // malformed tracker must produce a diagnostic rather than a stack trace, but a test
    // that cannot panic cannot fail.
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    /// The verb list and the help table are two lists of the same thing, and a verb that
    /// gains help nobody can reach is as useless as one with none.
    #[test]
    fn every_verb_the_binary_offers_has_help() {
        for verb in crate::cli::VERBS {
            assert!(for_verb(verb).is_some(), "`{verb}` has no help");
        }
    }

    /// Help that disagrees with the parser is worse than absent help, because it is
    /// believed. Both directions: nothing documented that would be refused, nothing
    /// accepted that goes unmentioned.
    #[test]
    fn documented_options_are_exactly_the_accepted_ones() {
        for (verb, _, accepted) in crate::cli::KNOWN_FLAGS {
            let Some(h) = HELP.iter().find(|h| h.verb == *verb) else {
                continue;
            };
            let documented: Vec<&str> = h.opts.iter().map(|(o, _)| o.split_whitespace().next().unwrap_or(o)).collect();
            for flag in &documented {
                assert!(accepted.contains(flag), "`{verb}` documents {flag}, which it would refuse");
            }
            // An alias documents nothing of its own; its options live with the verb it
            // stands for, and the rendering says where.
            if !h.alias_of.is_empty() {
                continue;
            }
            for flag in *accepted {
                // Explained once in the global section instead of two dozen times.
                if *flag == "--dir" {
                    continue;
                }
                assert!(documented.contains(flag), "`{verb}` accepts {flag} and does not document it");
            }
        }
    }

    #[test]
    fn the_rendering_carries_usage_blurb_and_every_option() {
        let text = for_verb("new").expect("new has help");
        assert!(text.contains("usage: trck new <title>"), "{text}");
        assert!(text.contains("--priority"), "{text}");
        assert!(text.contains("--requires"), "{text}");
        assert!(text.contains("example:"), "{text}");
        assert!(text.contains("--dir"), "global options missing: {text}");
    }

    /// The one place a line can run away is a long option description, since the term
    /// column is as wide as the widest flag.
    #[test]
    fn nothing_renders_wider_than_it_should() {
        for verb in crate::cli::VERBS {
            let text = for_verb(verb).expect("help");
            for line in text.lines() {
                assert!(line.chars().count() <= 100, "`{verb}` renders a {}-column line: {line}", line.chars().count());
            }
        }
    }

    #[test]
    fn an_unknown_verb_has_no_help_rather_than_empty_help() {
        assert!(for_verb("nonesuch").is_none());
    }
}

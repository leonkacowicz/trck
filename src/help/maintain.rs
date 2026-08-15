//! Help for checking, generating, and one-shot maintenance
//!
//! Grouped the way `cli::dispatch` groups the same verbs, so adding one touches the
//! matching pair rather than a table nobody can find the right end of.

use super::VerbHelp;

pub(super) const VERBS: &[VerbHelp] = &[
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
        verb: "sync",
        tagline: "share local tracker commits, and pick up what landed elsewhere",
        usage: "trck sync [options]",
        blurb: "Push what is waiting on the local tracker branch, then fetch. Reads never touch \
                the network, and a write that cannot reach the remote still succeeds because its \
                commit is anchored locally — so this is the one verb whose job is the network, and \
                the remedy every pending-changes note names. Nothing waiting and nothing new says so.",
        args: &[],
        opts: &[],
        alias_of: "",
        example: "trck sync",
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

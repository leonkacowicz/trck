//! What the parser knows about verbs and flags.
//!
//! Split from [`super::opts`], which turns arguments into one verb's options struct. These
//! are the tables every verb is checked *against* — which verbs exist, which flags each
//! takes, which take a value, how many positionals are required — and they belong to the
//! parser rather than to any one verb. They had accumulated in `opts` because it was the
//! nearest place to put them.
//!
//! [`KNOWN_FLAGS`] arrived from `mod.rs`, where it was the odd one out: every other per-verb
//! table was already here, and the guard that reads it — [`unrecognized_flag`] — was too. The
//! two files it is named from now name it from one place. `VALUED` and [`is_valued`] followed
//! for the same reason: which flags take a value is a fact about the flags, not about the
//! loop in `parse_args` that consults it.

use super::Args;

/// `list` and its `tree` alias take exactly the same options, named once.
pub(super) const LIST_FLAGS: &[&str] = &[
    "--dir",
    "--status",
    "--priority",
    "--label",
    "--parent",
    "--match",
    "--contains",
    "--field",
    "--show-field",
    "--sort",
    "--blocked",
    "--orphan",
    "--all",
    "--flat",
    "--paths",
    "--json",
];

/// Options that take a value. Anything not listed is a boolean flag, so
/// `trck list --flat --json` parses without the flags swallowing each other.
const VALUED: &[&str] = &[
    "--dir",
    "--body",
    "--body-file",
    "--ref",
    "--id",
    "--slug",
    "--priority",
    "--points",
    "--parent",
    "--spec",
    "--review-url",
    "--resolution",
    "--title",
    "--status",
    "--field",
    "--unset",
    "--add",
    "--remove",
    "--sort",
    "--label",
    "--show-field",
    "--match",
    "--contains",
    "--since",
    "--from",
    "--to",
    "--out",
    "--cmd",
    "--port",
    "--poll",
];

/// `--requires` is the one flag whose arity depends on the verb: `new --requires a,b` names
/// the issues a new one waits on, while `deps --requires` is a filter that takes nothing.
/// Same word, deliberately — they describe the same edges from either end — so the parser
/// resolves it against the verb rather than forcing one of them to be spelled differently.
///
/// Reading the verb first is safe because a flag before it can only be `--dir`, which is
/// unambiguously valued.
pub(super) fn is_valued(name: &str, verb: &str) -> bool {
    if name == "--requires" {
        return verb == "new";
    }
    VALUED.contains(&name)
}

/// The flags each verb accepts, so a typo is refused rather than ignored.
///
/// Silently dropping an unrecognised option is the worst of both worlds: `list
/// --stauts done` would list everything and read as a successful filter. Python's
/// argparse rejects it; so does this, though the message differs — argparse prints its
/// own usage block, and reproducing that is pinning argparse rather than trck.
pub(crate) const KNOWN_FLAGS: &[(&str, usize, &[&str])] = &[
    ("new", 0, &["--dir", "--id", "--slug", "--priority", "--points", "--parent", "--requires", "--spec", "--review-url", "--body", "--body-file", "--empty"]),
    ("edit", 0, super::prose::EDIT_FLAGS),
    ("sync", 0, &["--dir"]),
    ("mv", 0, &["--dir", "--resolution", "--review-url"]),
    ("start", 0, &["--dir"]),
    ("review", 0, &["--dir", "--review-url"]),
    ("done", 0, &["--dir", "--resolution"]),
    ("set", 0, &["--dir", "--auto", "--priority", "--points", "--parent", "--spec", "--review-url", "--title", "--slug", "--field", "--unset"]),
    ("dep", 0, &["--dir", "--add", "--remove"]),
    ("label", 0, &["--dir", "--add", "--remove"]),
    ("list", 0, LIST_FLAGS),
    // Named once rather than repeated: `tree` is an alias, so a flag either verb accepted
    // alone would be a flag the other silently refused.
    ("tree", 0, LIST_FLAGS),
    ("show", 0, &["--dir", "--json"]),
    ("path", 0, &["--dir"]),
    ("which", 0, &["--dir", "--ids"]),
    ("check", 0, &["--dir"]),
    ("html", 0, &["--dir", "--out", "--cmd"]),
    ("serve", 0, &["--dir", "--port", "--poll"]),
    ("diff", 0, &["--dir", "--from", "--to"]),
    ("changelog", 0, &["--dir", "--since"]),
    ("summary", 0, &["--dir"]),
    ("ready", 0, &["--dir", "--next", "--json"]),
    ("deps", 0, &["--dir", "--requires", "--blocks", "--full", "--omit-done", "--include-done-chains", "--fanout", "--json"]),
    ("next", 0, &["--dir", "--json"]),
    // `--no-vendor` is accepted and does nothing. It asked for the only behaviour there is
    // now, so refusing it would mean erroring on a request already satisfied; every README
    // and script that learned to pass it keeps working. `init -h` says as much.
    ("init", 1, &["--dir", "--force", "--hook", "--no-vendor"]),
];

/// Verbs whose `--json` is implemented. The rest still refuse the flag: accepted-and-ignored
/// returns human text with exit 0, and a caller piping into `jq` finds out far from the cause.
pub(super) const JSON_VERBS: &[&str] = &["list", "tree", "show", "ready", "next", "deps"];

pub(super) const MIN_POSITIONAL: &[(&str, usize, &str)] = &[
    ("new", 1, "a title"),
    ("mv", 2, "an issue id and a target status"),
    ("start", 1, "an issue id"),
    ("review", 1, "an issue id"),
    ("done", 1, "an issue id"),
    ("set", 1, "an issue id"),
    ("dep", 1, "an issue id"),
    ("label", 1, "an issue id"),
    ("show", 1, "an issue id"),
    ("path", 1, "an issue id"),
];

/// Options a verb cannot run without.
pub(super) const REQUIRED_OPTS: &[(&str, &str)] = &[("changelog", "--since")];

/// Everything this binary offers. It began as a list of what the Python engine had, so
/// `--help` could be honest about what was missing; it is now simply the verb list.
pub(crate) const VERBS: &[&str] = &[
    "new",
    "edit",
    "sync",
    "mv",
    "start",
    "review",
    "done",
    "set",
    "dep",
    "label",
    "show",
    "path",
    "which",
    "list",
    "tree",
    "ready",
    "next",
    "deps",
    "changelog",
    "diff",
    "check",
    "summary",
    // `html` has no Python counterpart: it is `tools/trck-html`, folded in. The verb
    // list is what this binary offers, not a mirror of the old CLI.
    "html",
    "serve",
    "repo",
    "init",
    "version",
];

/// The nth positional as an issue id, or the missing-operand error naming the verb.
///
/// One function rather than the same closure rebuilt in each dispatcher: three copies of a
/// message is three chances for them to drift apart.
pub(super) fn id_operand(args: &Args, n: usize) -> Result<&str, String> {
    args.positional_at(n).ok_or_else(|| format!("{}: missing an issue id", args.verb))
}

/// Flags every verb accepts, so they are not repeated two dozen times in [`KNOWN_FLAGS`].
///
/// `--dir` is still listed there as well, because the help test reads that table to check
/// that what is documented is what is accepted; the duplication is harmless and removing it
/// is a separate change.
pub(crate) const GLOBAL_FLAGS: &[&str] = &["--dir", "--ref"];

/// The first option this verb does not accept, if any.
///
/// Its own function because `usage_error` is a list of guards and this is the only one that
/// has to consult two tables: the verb's own flags, and the ones every verb takes.
pub(super) fn unrecognized_flag(args: &Args) -> Option<&str> {
    let (_, _, flags) = KNOWN_FLAGS.iter().find(|(verb, ..)| *verb == args.verb)?;
    args.options.iter().map(|(n, _)| n.as_str()).find(|n| !flags.contains(n) && !GLOBAL_FLAGS.contains(n))
}

//! What the parser knows about verbs and flags.
//!
//! Split from [`super::opts`], which turns arguments into one verb's options struct. These
//! are the tables every verb is checked *against* — which verbs exist, which flags each
//! takes, which take a value, how many positionals are required — and they belong to the
//! parser rather than to any one verb. They had accumulated in `opts` because it was the
//! nearest place to put them.

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
    let (_, _, flags) = super::KNOWN_FLAGS.iter().find(|(verb, ..)| *verb == args.verb)?;
    args.options.iter().map(|(n, _)| n.as_str()).find(|n| !flags.contains(n) && !GLOBAL_FLAGS.contains(n))
}

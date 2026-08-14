//! Turning parsed argv into the option structs the verbs take.
//!
//! Split from `dispatch` because it answers a different question: not *which* command to
//! run, but what that command was asked for. Each builder mirrors one verb's flags
//! one-to-one, which is why they are flat and repetitive — the shape is the point.

use super::{Args, KNOWN_FLAGS};
use crate::init;
use crate::query::{DepsOpts, ListOpts};
use crate::verbs::{MvOpts, NewOpts, SetOpts};

/// `mv`'s options — and, through `verb`, those of its three aliases.
///
/// `start`/`review`/`done` name the destination instead of taking it positionally, and
/// `review` takes the URL positionally too: the moment a review exists is the moment both
/// facts are known.
pub(super) fn mv_opts<'a>(args: &'a Args, verb: &str) -> Result<MvOpts<'a>, String> {
    let named = match crate::config::resolve_alias(verb) {
        Some(status) => status,
        None => args.positional_at(1).ok_or_else(|| "mv: missing a target status".to_string())?,
    };
    // A retired status name still moves the issue — to the status it was renamed to.
    let status = crate::config::canonical_status(named);
    let positional_url = (verb == "review").then(|| args.positional_at(1)).flatten();
    Ok(MvOpts { status, resolution: args.opt("--resolution"), review_url: positional_url.or_else(|| args.opt("--review-url")) })
}

/// `set`'s options.
pub(super) fn set_opts(args: &Args) -> Result<SetOpts<'_>, String> {
    Ok(SetOpts {
        auto: args.has("--auto"),
        priority: args.opt("--priority"),
        points: args.opt("--points").map(|v| v.parse().map_err(|_| format!("bad points '{v}'"))).transpose()?,
        parent: args.opt("--parent"),
        spec: args.opt("--spec"),
        review_url: args.opt("--review-url"),
        title: args.opt("--title"),
        slug: args.opt("--slug"),
        fields: args.all("--field"),
        unset: args.all("--unset"),
    })
}

/// `list`'s options. The flags map one-to-one, which is why there are so many booleans.
pub(super) fn list_opts(args: &Args) -> ListOpts<'_> {
    ListOpts {
        root: args.positional_at(0),
        status: args.opt("--status"),
        priority: args.opt("--priority"),
        label: args.opt("--label"),
        parent: args.opt("--parent"),
        match_title: args.opt("--match"),
        fields: args.all("--field"),
        show_fields: args.all("--show-field"),
        sort: args.opt("--sort"),
        blocked: args.has("--blocked"),
        orphan: args.has("--orphan"),
        all: args.has("--all"),
        flat: args.has("--flat"),
        paths: args.has("--paths"),
        json: args.has("--json"),
    }
}

/// `new`'s options.
pub(super) fn new_opts(args: &Args) -> Result<NewOpts, String> {
    let title = args.positional_at(0).ok_or_else(|| "new: missing a title".to_string())?;
    Ok(NewOpts {
        title: title.to_string(),
        id: args.opt("--id").map(str::to_string),
        slug: args.opt("--slug").map(str::to_string),
        priority: args.opt("--priority").map(str::to_string),
        points: args.opt("--points").map(|v| v.parse().map_err(|_| format!("bad points '{v}'"))).transpose()?,
        parent: args.opt("--parent").map(str::to_string),
        depends: args.opt("--requires").map(|v| v.split(',').map(str::trim).filter(|s| !s.is_empty()).map(str::to_string).collect()).unwrap_or_default(),
        spec: args.opt("--spec").map(str::to_string),
        review_url: args.opt("--review-url").map(str::to_string),
    })
}

/// `deps`'s options, which mirror its flags one-to-one.
pub(super) fn deps_opts(args: &Args) -> DepsOpts<'_> {
    DepsOpts {
        root: args.positional_at(0),
        requires: args.has("--requires"),
        blocks: args.has("--blocks"),
        full: args.has("--full"),
        omit_done: args.has("--omit-done"),
        include_done_chains: args.has("--include-done-chains"),
        fanout: args.has("--fanout"),
    }
}

/// Where `init` was told to put the tracker.
///
/// The positional and `--dir` mean the same thing, and giving both is refused rather than
/// silently preferring one — the Python engine's rule, kept because a caller that passes two
/// different directories has a bug worth being told about.
pub(super) fn init_from_args(args: &Args) -> Result<String, String> {
    let positional = args.positional.first().map(std::path::PathBuf::from);
    let flag = args.opt("--dir").map(std::path::PathBuf::from);
    if positional.is_some() && flag.is_some() {
        return Err("cannot combine a positional dir with --dir".to_string());
    }
    init::cmd_init(&init::InitOpts { target: positional.or(flag), force: args.has("--force"), hook: args.has("--hook") })
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

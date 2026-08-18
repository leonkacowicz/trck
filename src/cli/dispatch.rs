//! Verb dispatch: turning parsed arguments into a command and running it.
//!
//! Split from the parsing beside it because they answer different questions — what did the
//! user type, and what should that do — and because one `match` over two dozen verbs is a
//! function nobody can hold in their head. The groups (write something, read something, report
//! on the tracker, maintain the repository) are the seams the verbs already fall along.
//!
//! Two of those groups route from the file that implements them rather than from here —
//! `reports` and `maintain` — because in both cases the routing is a restatement of what that
//! file already knows, and one of them (`repo`) needs a paragraph per subcommand about which
//! context it may resolve. [`dispatch`] names the stages in order; adding a group is a name in
//! that list.

use super::opts::{deps_opts, init_from_args, list_opts, mv_opts, set_opts};
use super::reports::{cmd_summary, dispatch_reports};
use super::tables::{VERBS, id_operand};
use super::{Args, context, parse_args, tracker_dir};
use crate::query;
use crate::verbs;

/// The verbs that write. Returns `None` when the verb is not one of them, so `dispatch`
/// can fall through to the read side — the split is by what they do to the tracker, which
/// is also the split in how much can go wrong.
fn dispatch_mutating(args: &Args) -> Option<Result<String, String>> {
    let ctx = || context(args);
    let done = match args.verb.as_str() {
        verb @ ("mv" | "start" | "review" | "done") => ctx().and_then(|c| mv_opts(args, verb).and_then(|o| verbs::cmd_mv(&c, id_operand(args, 0)?, &o))),
        "set" => ctx().and_then(|c| set_opts(args).and_then(|o| verbs::cmd_set(&c, id_operand(args, 0)?, &o))),
        "label" => ctx().and_then(|c| verbs::cmd_label(&c, id_operand(args, 0)?, &args.all("--add"), &args.all("--remove"))),
        "dep" => ctx().and_then(|c| verbs::cmd_dep(&c, id_operand(args, 0)?, args.opt("--add"), args.opt("--remove"))),
        "summary" => ctx().and_then(|c| cmd_summary(&c)),
        _ => return None,
    };
    Some(noting_pending(args, done))
}

/// Append what a write left unshared, if anything.
///
/// A write that could not reach the remote succeeds — the commit is anchored on the local
/// branch, which is why it is written first — so the only thing left is to say so. Without
/// this the offline story is silent, and a silent unshared write is indistinguishable from a
/// shared one right up until someone else cannot see the issue.
///
/// Never on `--json`: that output has one consumer and it is a parser.
pub(super) fn noting_pending(args: &Args, done: Result<String, String>) -> Result<String, String> {
    let out = done?;
    if args.has("--json") {
        return Ok(out);
    }
    let Ok(crate::discovery::Source::Ref { cwd, .. }) = super::tracker::tracker_source(args) else {
        return Ok(out);
    };
    // A count that cannot be taken is not worth failing a completed write over.
    match crate::discovery::standing::pending(&cwd) {
        Ok(0) | Err(_) => Ok(out),
        Ok(n) => Ok(format!("{out}  ({n} unpushed change{} — run `trck sync`)", if n == 1 { "" } else { "s" })),
    }
}

/// Run the command described by `argv` (without the program name), returning what to
/// print on success.
/// The browse verbs: the ones that render a selection of issues, and the ones `--json`
/// applies to. Split from the rest of the read side along exactly that line — each arm here
/// picks between a human rendering and a machine one, and none of the others has that shape.
fn dispatch_browse(args: &Args) -> Option<Result<String, String>> {
    let json = args.has("--json");
    Some(match args.verb.as_str() {
        "list" | "tree" => context(args).and_then(|ctx| query::cmd_list(&ctx, &list_opts(args))),
        "show" => context(args).and_then(|ctx| {
            let id = id_operand(args, 0)?;
            if json { query::cmd_show_json(&ctx, id) } else { query::cmd_show(&ctx, id) }
        }),
        verb @ ("ready" | "next") => context(args).and_then(|ctx| {
            let only_next = verb == "next" || args.has("--next");
            let root = args.positional_at(0);
            if json { query::cmd_ready_json(&ctx, root, only_next) } else { query::cmd_ready(&ctx, root, only_next) }
        }),
        "deps" => context(args).and_then(|ctx| {
            let opts = deps_opts(args);
            if json { query::cmd_deps_json(&ctx, &opts) } else { query::cmd_deps(&ctx, &opts) }
        }),
        _ => return None,
    })
}

/// The read verbs that are not a listing and not a report: resolve the tracker, answer once.
fn dispatch_query(args: &Args) -> Option<Result<String, String>> {
    if let Some(result) = dispatch_browse(args) {
        return Some(result);
    }
    Some(match args.verb.as_str() {
        "path" => context(args).and_then(|ctx| query::cmd_path(&ctx, id_operand(args, 0)?)),
        "which" => context(args).and_then(|ctx| {
            let paths = query::which_operands(&args.positional)?;
            query::cmd_which(&ctx, &paths, args.has("--ids"))
        }),
        "html" => context(args).and_then(|ctx| crate::html::cmd_html(&ctx, args.opt("--out"), args.opt("--cmd"))),
        // The one verb that does not return: it prints where it is listening and then serves
        // until the process is signalled. The tracker is resolved before anything is bound, so
        // an unresolvable one refuses here rather than leaving a socket listening on a page
        // the process cannot render.
        "serve" => context(args).and_then(|ctx| crate::serve::cmd_serve(&ctx, args.opt("--port"), args.opt("--poll"))),
        _ => return None,
    })
}

pub(super) fn dispatch(raw: &[String]) -> Result<String, String> {
    let args = parse_args(raw);
    // In order, first to claim the verb wins. A loop rather than a chain of `if let`s so
    // that adding a group is a line in the list rather than a change to the control flow.
    for stage in [super::prose::dispatch_prose, super::sync::dispatch_sync, dispatch_mutating, dispatch_query, dispatch_reports] {
        if let Some(result) = stage(&args) {
            return result;
        }
    }
    match args.verb.as_str() {
        "repo" => super::maintain::dispatch_repo(&args),
        // Version on stdout, tracker on stderr — so `trck version` stays pipeable to
        // something that wants only the number while a human still sees which tracker
        // they are pointed at.
        "version" => {
            if let Ok(dir) = tracker_dir(&args) {
                eprintln!("tracker: {}", dir.display());
            }
            Ok(env!("CARGO_PKG_VERSION").to_string())
        },
        // The one verb that runs without a tracker: it takes its target rather than
        // discovering one, so it never goes through `context`.
        "init" => init_from_args(&args),
        "" => Err("no verb given".to_string()),
        other if VERBS.contains(&other) => Err(format!("`{other}` is not implemented yet in the Rust engine")),
        other => Err(format!("unknown verb `{other}`")),
    }
}

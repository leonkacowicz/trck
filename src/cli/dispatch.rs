//! Verb dispatch: turning parsed arguments into a command and running it.
//!
//! Split from the parsing beside it because they answer different questions — what did the
//! user type, and what should that do — and because one `match` over two dozen verbs is a
//! function nobody can hold in their head. The three groups here (write something, read
//! something, maintain the repository) are the seams the verbs already fall along.

use super::{Args, VERBS, context, emit, is_closed_pipe, parse_args, tracker_dir};
use crate::config;
use crate::discovery::Ctx;
use crate::init;
use crate::query::ListOpts;
use crate::query::{self, DepsOpts};
use crate::repo;
use crate::verbs;
use crate::verbs::{NewOpts, SetOpts};

/// The verbs that write. Returns `None` when the verb is not one of them, so `dispatch`
/// can fall through to the read side — the split is by what they do to the tracker, which
/// is also the split in how much can go wrong.
fn dispatch_mutating(args: &Args) -> Option<Result<String, String>> {
    let id_of = |n: usize| -> Result<&str, String> { args.positional_at(n).ok_or_else(|| format!("{}: missing an issue id", args.verb)) };
    let ctx = || context(args);
    Some(match args.verb.as_str() {
        "new" => ctx().and_then(|c| new_opts(args).and_then(|o| verbs::cmd_new(&c, &o))),
        "mv" => ctx().and_then(|c| {
            let status = args.positional_at(1).ok_or_else(|| "mv: missing a target status".to_string())?;
            verbs::cmd_mv(&c, id_of(0)?, status, args.opt("--resolution"), args.opt("--review-url"))
        }),
        verb @ ("start" | "review" | "done") => ctx().and_then(|c| {
            let status = config::resolve_alias(verb).unwrap_or(config::BACKLOG);
            // `review` takes the URL positionally: the moment a review exists is the
            // moment both facts are known.
            let url = if verb == "review" { args.positional_at(1).or_else(|| args.opt("--review-url")) } else { args.opt("--review-url") };
            verbs::cmd_mv(&c, id_of(0)?, status, args.opt("--resolution"), url)
        }),
        "set" => ctx().and_then(|c| set_opts(args).and_then(|o| verbs::cmd_set(&c, id_of(0)?, &o))),
        "label" => ctx().and_then(|c| verbs::cmd_label(&c, id_of(0)?, &args.all("--add"), &args.all("--remove"))),
        "dep" => ctx().and_then(|c| verbs::cmd_dep(&c, id_of(0)?, args.opt("--add"), args.opt("--remove"))),
        "summary" => ctx().and_then(|c| {
            let rows = verbs::load_rows(&c)?;
            let g = crate::graph::Graph::new(rows);
            let n = g.rows.len();
            verbs::write_file(&c.summary_path(), &crate::summary::generate_summary(&g))?;
            Ok(format!("wrote {} ({n} issues)", c.summary_path().display()))
        }),
        _ => return None,
    })
}

/// Run the command described by `argv` (without the program name), returning what to
/// print on success.
/// The read verbs. Split out for the same reason `dispatch_mutating` is: one `match` over
/// two dozen verbs is a function nobody can hold in their head, and the three groups —
/// change something, read something, maintain the repository — are the natural seams.
fn dispatch_query(args: &Args) -> Option<Result<String, String>> {
    let id_of = |n: usize| -> Result<&str, String> { args.positional_at(n).ok_or_else(|| format!("{}: missing an issue id", args.verb)) };
    let json = args.has("--json");
    Some(match args.verb.as_str() {
        "list" | "tree" => context(args).and_then(|ctx| query::cmd_list(&ctx, &list_opts(args))),
        "show" => context(args).and_then(|ctx| {
            let id = id_of(0)?;
            if json { query::cmd_show_json(&ctx, id) } else { query::cmd_show(&ctx, id) }
        }),
        verb @ ("ready" | "next") => context(args).and_then(|ctx| {
            let only_next = verb == "next" || args.has("--next");
            let root = args.positional_at(0);
            if json { query::cmd_ready_json(&ctx, root, only_next) } else { query::cmd_ready(&ctx, root, only_next) }
        }),
        "deps" => context(args).and_then(|ctx| {
            let opts = DepsOpts {
                root: args.positional_at(0),
                requires: args.has("--requires"),
                blocks: args.has("--blocks"),
                full: args.has("--full"),
                omit_done: args.has("--omit-done"),
                include_done_chains: args.has("--include-done-chains"),
                fanout: args.has("--fanout"),
            };
            if json { query::cmd_deps_json(&ctx, &opts) } else { query::cmd_deps(&ctx, &opts) }
        }),
        "html" => context(args).and_then(|ctx| crate::html::cmd_html(&ctx, args.opt("--out"), args.opt("--cmd"))),
        "diff" => context(args).and_then(|ctx| cmd_diff(&ctx, args)),
        "changelog" => context(args).and_then(|ctx| cmd_changelog(&ctx, args)),
        "check" => context(args).and_then(|ctx| cmd_check(&ctx)),
        _ => return None,
    })
}

/// `repo` and its subcommands.
///
/// The context is resolved per subcommand rather than once, because they disagree about
/// what they need: the merge drivers must work with no tracker in reach at all, and
/// `migrate-layout` must reach one the ordinary guards would refuse.
fn dispatch_repo(args: &Args) -> Result<String, String> {
    let sub = args.positional_at(0).unwrap_or("");
    let operand = |n: usize| -> Result<&str, String> { args.positional_at(n).ok_or_else(|| format!("repo {sub}: missing operand {n}")) };
    match sub {
        // git may invoke a driver from anywhere in the worktree, and a merge with no
        // reachable trck.json still has to merge the rows it was handed.
        "merge-index" => repo::cmd_merge_index(context(args).ok().as_ref(), operand(1)?, operand(2)?, operand(3)?),
        "merge-summary" => repo::cmd_merge_summary(context(args).ok().as_ref(), operand(1)?),
        // These need a real tracker, unlike the drivers.
        "setup-git" => repo::cmd_setup_git(&context(args)?),
        "install-hook" => repo::cmd_install_hook(&context(args)?),
        "normalize" => repo::cmd_normalize(&context(args)?),
        // The one verb whose whole job is to operate on a legacy tracker, so it resolves
        // the context without the layout guard that refuses one.
        "migrate-layout" => repo::cmd_migrate_layout(&Ctx::load(tracker_dir(args)?, false)?, args.has("--dry-run")),
        "" => Err("repo: missing a subcommand".into()),
        other => Err(format!("repo: `{other}` is not implemented yet in the Rust engine")),
    }
}

pub(super) fn dispatch(raw: &[String]) -> Result<String, String> {
    let args = parse_args(raw);
    if let Some(result) = dispatch_mutating(&args) {
        return result;
    }
    if let Some(result) = dispatch_query(&args) {
        return result;
    }
    match args.verb.as_str() {
        "repo" => dispatch_repo(&args),
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

/// Where `init` was told to put the tracker.
///
/// The positional and `--dir` mean the same thing, and giving both is refused rather than
/// silently preferring one — the Python engine's rule, kept because a caller that passes two
/// different directories has a bug worth being told about.
fn init_from_args(args: &Args) -> Result<String, String> {
    let positional = args.positional.first().map(std::path::PathBuf::from);
    let flag = args.opt("--dir").map(std::path::PathBuf::from);
    if positional.is_some() && flag.is_some() {
        return Err("cannot combine a positional dir with --dir".to_string());
    }
    init::cmd_init(&init::InitOpts { target: positional.or(flag), force: args.has("--force"), hook: args.has("--hook") })
}

/// What shipped since a cutoff.
fn cmd_changelog(ctx: &Ctx, args: &Args) -> Result<String, String> {
    // `usage_error` has already established --since is present.
    let since = crate::diff::parse_since(args.opt("--since").unwrap_or_default())?;
    let rows = verbs::load_rows(ctx)?;
    let shipped = crate::diff::select_shipped(&rows, &since);
    Ok(crate::diff::render_changelog(&shipped, &since).trim_end_matches('\n').to_string())
}

/// Compare the tracker at two points and report what changed.
///
/// A bare revision spec goes through git; `--from`/`--to` name sources directly and never
/// touch it. With neither, the default is HEAD versus the working tree — "what have I not
/// committed?" — which is the git path too.
///
/// The output is deliberately minimal, one plain line per changed issue. The real layouts
/// are separate issues in the Python engine and are not ported ahead of it: see #2w5panf.
fn cmd_diff(ctx: &Ctx, args: &Args) -> Result<String, String> {
    let (old, new) = if let Some(rev) = args.positional_at(0) {
        let (old_rev, new_rev) = crate::diff::parse_rev_spec(rev)?;
        let old = crate::diff::git_snapshot(ctx, &old_rev)?;
        let new = match new_rev {
            Some(r) => crate::diff::git_snapshot(ctx, &r)?,
            None => crate::diff::resolve_source(args.opt("--to"), ctx)?,
        };
        (old, new)
    } else if let Some(from) = args.opt("--from") {
        (crate::diff::resolve_source(Some(from), ctx)?, crate::diff::resolve_source(args.opt("--to"), ctx)?)
    } else {
        (crate::diff::git_snapshot(ctx, "HEAD")?, crate::diff::resolve_source(args.opt("--to"), ctx)?)
    };
    let changes = crate::diff::diff_snapshots(&old, &new);
    let mut out = vec![format!("{} → {}", old.label, new.label)];
    if changes.is_empty() {
        out.push("no changes".into());
        return Ok(out.join("\n"));
    }
    for c in &changes {
        let sigil = match c.kind {
            "added" => "+",
            "removed" => "-",
            _ => "~",
        };
        let detail = match c.kind {
            "added" => "new".to_string(),
            "removed" => "removed".to_string(),
            _ => crate::diff::change_summary(c),
        };
        out.push(format!("{sigil} #{} {detail} — {}", c.id, c.title));
    }
    Ok(out.join("\n"))
}

/// `check` prints its findings on stdout and fails on any error. The report *is* the
/// message, so a failing run returns an empty error rather than a second diagnostic.
fn cmd_check(ctx: &Ctx) -> Result<String, String> {
    let rows = verbs::load_rows(ctx)?;
    let report = crate::validate::validate(ctx, &rows)?;
    let mut out: Vec<String> = report.warnings.iter().map(|w| format!("warning: {w}")).collect();
    out.extend(report.errors.iter().map(|e| format!("error: {e}")));
    if report.errors.is_empty() {
        out.push(format!("OK — {} issues, 0 errors, {} warning(s)", rows.len(), report.warnings.len()));
        return Ok(out.join("\n"));
    }
    out.push(String::new());
    out.push(format!("{} error(s), {} warning(s) — FAIL", report.errors.len(), report.warnings.len()));
    // Printed here rather than returned because this path exits non-zero with its report on
    // *stdout*, which the `Err` arm of `main` cannot express. A reader that has gone away
    // still must not turn a failed check into a panic.
    if let Err(ref e) = emit(&(out.join("\n") + "\n")) {
        if !is_closed_pipe(e) {
            eprintln!("error: writing output: {e}");
        }
    }
    Err(String::new())
}

/// `set`'s options.
fn set_opts(args: &Args) -> Result<SetOpts<'_>, String> {
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
fn list_opts(args: &Args) -> ListOpts<'_> {
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
fn new_opts(args: &Args) -> Result<NewOpts, String> {
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

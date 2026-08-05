//! Argument parsing and dispatch.
//!
//! Hand-written, because the engine takes no dependencies. That is affordable here for
//! one reason: the surface is small and stable. Options are `--flag value` or
//! `--flag=value`, everything else is positional, and `--` is not special because no
//! trck argument can be mistaken for a flag.
//!
//! Only the mutating verbs are wired so far. Anything else exits non-zero saying so,
//! which is what keeps the conformance pass rate an honest number rather than a
//! half-implemented verb quietly producing wrong output.

use crate::config;
use crate::discovery::Ctx;
use crate::query::{self, DepsOpts, ListOpts};
use crate::verbs::{self, NewOpts, SetOpts};

/// Verbs the Python engine has, so `--help` is honest about what is missing.
const VERBS: &[&str] = &[
    "new",
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
    "repo",
    "init",
    "update",
    "version",
];

/// Parsed argv: the verb, its positionals, and its options.
struct Args {
    verb: String,
    positional: Vec<String>,
    options: Vec<(String, Option<String>)>,
}

/// Options that take a value. Anything not listed is a boolean flag, so
/// `trck list --flat --json` parses without the flags swallowing each other.
const VALUED: &[&str] = &[
    "--dir",
    "--id",
    "--slug",
    "--priority",
    "--points",
    "--parent",
    "--depends",
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
    "--since",
];

fn parse_args(argv: &[String]) -> Args {
    let mut out = Args {
        verb: String::new(),
        positional: Vec::new(),
        options: Vec::new(),
    };
    let mut i = 0;
    while i < argv.len() {
        let a = &argv[i];
        if let Some(rest) = a.strip_prefix("--") {
            let (name, inline) = match rest.split_once('=') {
                Some((n, v)) => (format!("--{n}"), Some(v.to_string())),
                None => (format!("--{rest}"), None),
            };
            let value = if inline.is_some() {
                inline
            } else if VALUED.contains(&name.as_str()) && i + 1 < argv.len() {
                i += 1;
                Some(argv[i].clone())
            } else {
                None
            };
            out.options.push((name, value));
        } else if out.verb.is_empty() {
            out.verb.clone_from(a);
        } else {
            out.positional.push(a.clone());
        }
        i += 1;
    }
    out
}

impl Args {
    fn opt(&self, name: &str) -> Option<&str> {
        self.options
            .iter()
            .rev()
            .find(|(n, _)| n == name)
            .and_then(|(_, v)| v.as_deref())
    }

    fn all(&self, name: &str) -> Vec<&str> {
        self.options
            .iter()
            .filter(|(n, _)| n == name)
            .filter_map(|(_, v)| v.as_deref())
            .collect()
    }

    fn has(&self, name: &str) -> bool {
        self.options.iter().any(|(n, _)| n == name)
    }

    fn positional_at(&self, n: usize) -> Option<&str> {
        self.positional.get(n).map(String::as_str)
    }
}

/// The flags each verb accepts, so a typo is refused rather than ignored.
///
/// Silently dropping an unrecognised option is the worst of both worlds: `list
/// --stauts done` would list everything and read as a successful filter. Python's
/// argparse rejects it; so does this, though the message differs — argparse prints its
/// own usage block, and reproducing that is pinning argparse rather than trck.
const KNOWN_FLAGS: &[(&str, usize, &[&str])] = &[
    (
        "new",
        0,
        &[
            "--dir",
            "--id",
            "--slug",
            "--priority",
            "--points",
            "--parent",
            "--depends",
            "--spec",
            "--review-url",
        ],
    ),
    ("mv", 0, &["--dir", "--resolution", "--review-url"]),
    ("start", 0, &["--dir"]),
    ("review", 0, &["--dir", "--review-url"]),
    ("done", 0, &["--dir", "--resolution"]),
    (
        "set",
        0,
        &[
            "--dir",
            "--auto",
            "--priority",
            "--points",
            "--parent",
            "--spec",
            "--review-url",
            "--title",
            "--slug",
            "--field",
            "--unset",
        ],
    ),
    ("dep", 0, &["--dir", "--add", "--remove"]),
    ("label", 0, &["--dir", "--add", "--remove"]),
    (
        "list",
        0,
        &[
            "--dir",
            "--status",
            "--priority",
            "--label",
            "--parent",
            "--match",
            "--field",
            "--show-field",
            "--sort",
            "--blocked",
            "--orphan",
            "--all",
            "--flat",
            "--paths",
            "--json",
        ],
    ),
    (
        "tree",
        0,
        &[
            "--dir",
            "--status",
            "--priority",
            "--label",
            "--parent",
            "--match",
            "--field",
            "--show-field",
            "--sort",
            "--blocked",
            "--orphan",
            "--all",
            "--flat",
            "--paths",
            "--json",
        ],
    ),
    ("show", 0, &["--dir", "--json"]),
    ("check", 0, &["--dir"]),
    ("summary", 0, &["--dir"]),
    ("ready", 0, &["--dir", "--next", "--json"]),
    (
        "deps",
        0,
        &[
            "--dir",
            "--requires",
            "--blocks",
            "--full",
            "--omit-done",
            "--include-done-chains",
            "--fanout",
            "--json",
        ],
    ),
    ("next", 0, &["--dir", "--json"]),
];

/// How many positionals each verb requires.
const MIN_POSITIONAL: &[(&str, usize, &str)] = &[
    ("new", 1, "a title"),
    ("mv", 2, "an issue id and a target status"),
    ("start", 1, "an issue id"),
    ("review", 1, "an issue id"),
    ("done", 1, "an issue id"),
    ("set", 1, "an issue id"),
    ("dep", 1, "an issue id"),
    ("label", 1, "an issue id"),
    ("show", 1, "an issue id"),
];

/// Everything wrong with the *shape* of the invocation, as opposed to what it asks for.
///
/// Kept separate because it exits 2, not 1. That is argparse's convention and it is a
/// real distinction: a script can tell "you called me wrong" from "the thing you asked
/// for failed".
fn usage_error(args: &Args) -> Option<String> {
    if !args.verb.is_empty() && !VERBS.contains(&args.verb.as_str()) {
        return Some(format!("unknown verb `{}`", args.verb));
    }
    if let Some((_, _, flags)) = KNOWN_FLAGS.iter().find(|(verb, ..)| *verb == args.verb)
        && let Some(n) = args
            .options
            .iter()
            .map(|(n, _)| n.as_str())
            .find(|n| !flags.contains(n))
    {
        return Some(format!("{}: unrecognized argument {n}", args.verb));
    }
    if let Some((_, want, what)) = MIN_POSITIONAL.iter().find(|(verb, ..)| *verb == args.verb)
        && args.positional.len() < *want
    {
        return Some(format!("{}: missing {what}", args.verb));
    }
    None
}

fn usage() -> String {
    format!(
        "trck {} (Rust) — deterministic in-repo issue tracker\n\
         \n\
         The port is in progress: the mutating verbs work, the read verbs do not yet.\n\
         Progress is measured by conformance/, not described here.\n\
         \n\
         Verbs: {}\n",
        env!("CARGO_PKG_VERSION"),
        VERBS.join(", ")
    )
}

/// Resolve the tracker and load it, applying the format guard.
fn context(args: &Args) -> Result<Ctx, String> {
    let cwd =
        std::env::current_dir().map_err(|e| format!("cannot read the working directory: {e}"))?;
    let self_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(std::path::Path::to_path_buf));
    let env_dir = std::env::var("TRCK_DIR").ok().filter(|v| !v.is_empty());
    let dir = crate::discovery::resolve_tracker_dir(
        args.opt("--dir"),
        env_dir.as_deref(),
        self_dir.as_deref(),
        &cwd,
    )?;
    Ctx::load(dir, true)
}

/// Run the command described by `argv` (without the program name), returning what to
/// print on success.
fn dispatch(raw: &[String]) -> Result<String, String> {
    let args = parse_args(raw);
    let id_of = |n: usize| -> Result<&str, String> {
        args.positional_at(n)
            .ok_or_else(|| format!("{}: missing an issue id", args.verb))
    };

    match args.verb.as_str() {
        "new" => {
            let ctx = context(&args)?;
            let opts = new_opts(&args)?;
            verbs::cmd_new(&ctx, &opts)
        }
        "mv" => {
            let ctx = context(&args)?;
            let status = args
                .positional_at(1)
                .ok_or_else(|| "mv: missing a target status".to_string())?;
            verbs::cmd_mv(
                &ctx,
                id_of(0)?,
                status,
                args.opt("--resolution"),
                args.opt("--review-url"),
            )
        }
        verb @ ("start" | "review" | "done") => {
            let ctx = context(&args)?;
            let status = config::resolve_alias(verb).unwrap_or(config::BACKLOG);
            // `review` takes the URL positionally, because the moment a review exists
            // is the moment both facts are known.
            let url = if verb == "review" {
                args.positional_at(1).or_else(|| args.opt("--review-url"))
            } else {
                args.opt("--review-url")
            };
            verbs::cmd_mv(&ctx, id_of(0)?, status, args.opt("--resolution"), url)
        }
        "set" => {
            let ctx = context(&args)?;
            let opts = set_opts(&args)?;
            verbs::cmd_set(&ctx, id_of(0)?, &opts)
        }
        "label" => {
            let ctx = context(&args)?;
            verbs::cmd_label(&ctx, id_of(0)?, &args.all("--add"), &args.all("--remove"))
        }
        "dep" => {
            let ctx = context(&args)?;
            verbs::cmd_dep(&ctx, id_of(0)?, args.opt("--add"), args.opt("--remove"))
        }
        "list" | "tree" => {
            let ctx = context(&args)?;
            query::cmd_list(&ctx, &list_opts(&args))
        }
        verb @ ("ready" | "next") => {
            let ctx = context(&args)?;
            query::cmd_ready(
                &ctx,
                args.positional_at(0),
                verb == "next" || args.has("--next"),
            )
        }
        "check" => {
            let ctx = context(&args)?;
            cmd_check(&ctx)
        }
        "summary" => {
            let ctx = context(&args)?;
            let rows = verbs::load_rows(&ctx)?;
            let g = crate::graph::Graph::new(rows);
            let n = g.rows.len();
            verbs::write_file(&ctx.summary_path(), &crate::summary::generate_summary(&g))?;
            Ok(format!(
                "wrote {} ({n} issues)",
                ctx.summary_path().display()
            ))
        }
        "deps" => {
            let ctx = context(&args)?;
            let opts = DepsOpts {
                root: args.positional_at(0),
                requires: args.has("--requires"),
                blocks: args.has("--blocks"),
                full: args.has("--full"),
                omit_done: args.has("--omit-done"),
                include_done_chains: args.has("--include-done-chains"),
                fanout: args.has("--fanout"),
            };
            query::cmd_deps(&ctx, &opts)
        }
        "show" => {
            let ctx = context(&args)?;
            query::cmd_show(&ctx, id_of(0)?)
        }
        "" => Err("no verb given".to_string()),
        other if VERBS.contains(&other) => Err(format!(
            "`{other}` is not implemented yet in the Rust engine"
        )),
        other => Err(format!("unknown verb `{other}`")),
    }
}

/// `check` prints its findings on stdout and fails on any error. The report *is* the
/// message, so a failing run returns an empty error rather than a second diagnostic.
fn cmd_check(ctx: &Ctx) -> Result<String, String> {
    let rows = verbs::load_rows(ctx)?;
    let report = crate::validate::validate(ctx, &rows)?;
    let mut out: Vec<String> = report
        .warnings
        .iter()
        .map(|w| format!("warning: {w}"))
        .collect();
    out.extend(report.errors.iter().map(|e| format!("error: {e}")));
    if report.errors.is_empty() {
        out.push(format!(
            "OK — {} issues, 0 errors, {} warning(s)",
            rows.len(),
            report.warnings.len()
        ));
        return Ok(out.join("\n"));
    }
    out.push(String::new());
    out.push(format!(
        "{} error(s), {} warning(s) — FAIL",
        report.errors.len(),
        report.warnings.len()
    ));
    println!("{}", out.join("\n"));
    Err(String::new())
}

/// `set`'s options.
fn set_opts(args: &Args) -> Result<SetOpts<'_>, String> {
    Ok(SetOpts {
        auto: args.has("--auto"),
        priority: args.opt("--priority"),
        points: args
            .opt("--points")
            .map(|v| v.parse().map_err(|_| format!("bad points '{v}'")))
            .transpose()?,
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
    }
}

/// `new`'s options.
fn new_opts(args: &Args) -> Result<NewOpts, String> {
    let title = args
        .positional_at(0)
        .ok_or_else(|| "new: missing a title".to_string())?;
    Ok(NewOpts {
        title: title.to_string(),
        id: args.opt("--id").map(str::to_string),
        slug: args.opt("--slug").map(str::to_string),
        priority: args.opt("--priority").map(str::to_string),
        points: args
            .opt("--points")
            .map(|v| v.parse().map_err(|_| format!("bad points '{v}'")))
            .transpose()?,
        parent: args.opt("--parent").map(str::to_string),
        depends: args
            .opt("--depends")
            .map(|v| {
                v.split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default(),
        spec: args.opt("--spec").map(str::to_string),
        review_url: args.opt("--review-url").map(str::to_string),
    })
}

/// Entry point. Returns the process status; everything user-facing goes to stdout on
/// success and stderr on failure, matching the Python engine's `die`.
pub(crate) fn main() -> std::process::ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    if argv.is_empty() || argv.iter().any(|a| a == "-h" || a == "--help") {
        print!("{}", usage());
        return std::process::ExitCode::SUCCESS;
    }
    if let Some(msg) = usage_error(&parse_args(&argv)) {
        eprintln!("error: {msg}");
        return std::process::ExitCode::from(2);
    }
    match dispatch(&argv) {
        Ok(out) => {
            if !out.is_empty() {
                println!("{out}");
            }
            std::process::ExitCode::SUCCESS
        }
        Err(msg) if msg.is_empty() => std::process::ExitCode::FAILURE,
        Err(msg) => {
            eprintln!("error: {msg}");
            std::process::ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    // Tests assert; that is their job. The crate denies unwrap/expect/panic because a
    // malformed tracker must produce a diagnostic rather than a stack trace, but a test
    // that cannot panic cannot fail.
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    fn args(argv: &[&str]) -> Args {
        parse_args(&argv.iter().map(|s| (*s).to_string()).collect::<Vec<_>>())
    }

    #[test]
    fn a_verb_with_positionals_and_options() {
        let a = args(&[
            "new",
            "Fix the parser",
            "--priority",
            "high",
            "--id",
            "aaaaaaa",
        ]);
        assert_eq!(a.verb, "new");
        assert_eq!(a.positional_at(0), Some("Fix the parser"));
        assert_eq!(a.opt("--priority"), Some("high"));
        assert_eq!(a.opt("--id"), Some("aaaaaaa"));
    }

    #[test]
    fn equals_form_and_space_form_agree() {
        assert_eq!(
            args(&["mv", "a", "done", "--resolution=wontfix"]).opt("--resolution"),
            args(&["mv", "a", "done", "--resolution", "wontfix"]).opt("--resolution")
        );
    }

    #[test]
    fn a_boolean_flag_does_not_swallow_the_next_one() {
        // The reason `VALUED` exists: without it `--flat` would eat `--json`.
        let a = args(&["list", "--flat", "--json"]);
        assert_eq!(a.opt("--flat"), None);
        assert!(a.options.iter().any(|(n, _)| n == "--json"));
        assert!(a.positional.is_empty());
    }

    #[test]
    fn a_repeatable_option_keeps_every_value() {
        let a = args(&["set", "x", "--field", "a=1", "--field", "b=2"]);
        assert_eq!(a.all("--field"), ["a=1", "b=2"]);
    }

    #[test]
    fn the_global_dir_flag_is_read_wherever_it_sits() {
        // The Python CLI takes it before the verb; a fixture may write it either way.
        assert_eq!(
            args(&["--dir", "issues", "new", "T"]).opt("--dir"),
            Some("issues")
        );
        assert_eq!(
            args(&["new", "T", "--dir", "issues"]).opt("--dir"),
            Some("issues")
        );
        assert_eq!(args(&["--dir", "issues", "new", "T"]).verb, "new");
    }

    #[test]
    fn an_unrecognised_flag_is_refused_rather_than_ignored() {
        // `list --stauts done` would otherwise list everything and read as a filter.
        let bad = parse_args(&["list".into(), "--stauts".into(), "done".into()]);
        assert!(
            usage_error(&bad).is_some_and(|m| m.contains("unrecognized argument --stauts")),
            "a typo'd flag must be refused, not dropped"
        );
        // A flag the verb does accept passes.
        assert_eq!(
            usage_error(&parse_args(&["list".into(), "--all".into()])),
            None
        );
    }

    #[test]
    fn a_missing_positional_is_a_usage_error_not_an_operation_that_fails() {
        // It exits 2, like argparse: a script can tell "you called me wrong" from
        // "what you asked for failed".
        assert!(usage_error(&parse_args(&["show".into()])).is_some_and(|m| m.contains("missing")));
        assert!(
            usage_error(&parse_args(&["mv".into(), "abc".into()]))
                .is_some_and(|m| m.contains("missing"))
        );
    }

    #[test]
    fn an_unimplemented_verb_says_so_rather_than_guessing() {
        // `changelog` is still unported. Saying so beats producing something
        // approximate: a half-implemented verb is what would turn the conformance pass
        // rate into a lie. (This named `list`, then `deps`, as each landed — which is
        // the intended churn: the test tracks the frontier.)
        let err = dispatch(&["changelog".to_string()]).expect_err("not implemented");
        assert!(err.contains("not implemented"), "{err}");
        let unknown = usage_error(&parse_args(&["nonesuch".to_string()]));
        assert!(unknown.is_some_and(|m| m.contains("unknown verb")));
    }
}

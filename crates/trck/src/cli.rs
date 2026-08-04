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

    fn positional_at(&self, n: usize) -> Option<&str> {
        self.positional.get(n).map(String::as_str)
    }
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
            let title = args
                .positional_at(0)
                .ok_or_else(|| "new: missing a title".to_string())?;
            let opts = NewOpts {
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
            };
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
            let opts = SetOpts {
                auto: args.options.iter().any(|(n, _)| n == "--auto"),
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
            };
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
        "" => Err("no verb given".to_string()),
        other if VERBS.contains(&other) => Err(format!(
            "`{other}` is not implemented yet in the Rust engine"
        )),
        other => Err(format!("unknown verb `{other}`")),
    }
}

/// Entry point. Returns the process status; everything user-facing goes to stdout on
/// success and stderr on failure, matching the Python engine's `die`.
pub(crate) fn main() -> std::process::ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    if argv.is_empty() || argv.iter().any(|a| a == "-h" || a == "--help") {
        print!("{}", usage());
        return std::process::ExitCode::SUCCESS;
    }
    match dispatch(&argv) {
        Ok(out) => {
            if !out.is_empty() {
                println!("{out}");
            }
            std::process::ExitCode::SUCCESS
        }
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
    fn an_unimplemented_verb_says_so_rather_than_guessing() {
        let err = dispatch(&["list".to_string()]).expect_err("not implemented");
        assert!(err.contains("not implemented"), "{err}");
        let err = dispatch(&["nonesuch".to_string()]).expect_err("unknown");
        assert!(err.contains("unknown verb"), "{err}");
    }
}

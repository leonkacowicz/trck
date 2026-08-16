//! Argument parsing and dispatch.
//!
//! Hand-written, because the engine takes no dependencies. That is affordable here for
//! one reason: the surface is small and stable. Options are `--flag value` or
//! `--flag=value`, everything else is positional, and `--` is not special because no
//! trck argument can be mistaken for a flag.
//!
//! Every verb is wired. While the port was in flight the unwired ones exited non-zero
//! saying so, rather than producing something approximate — that is what kept the
//! conformance pass rate an honest number. The guard remains for `repo`, whose
//! subcommand list can still grow.

mod body;
mod dispatch;
mod editor;
pub(crate) mod opts;
mod prose;
mod reports;
mod sync;
pub(crate) mod tables;
mod tracker;
use dispatch::dispatch;
use tables::unrecognized_flag;
use tracker::{context, setup_source, tracker_dir};

use crate::help;

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
];

/// `--requires` is the one flag whose arity depends on the verb: `new --requires a,b` names
/// the issues a new one waits on, while `deps --requires` is a filter that takes nothing.
/// Same word, deliberately — they describe the same edges from either end — so the parser
/// resolves it against the verb rather than forcing one of them to be spelled differently.
///
/// Reading the verb first is safe because a flag before it can only be `--dir`, which is
/// unambiguously valued.
fn is_valued(name: &str, verb: &str) -> bool {
    if name == "--requires" {
        return verb == "new";
    }
    VALUED.contains(&name)
}

fn parse_args(argv: &[String]) -> Args {
    let mut out = Args { verb: String::new(), positional: Vec::new(), options: Vec::new() };
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
            } else if is_valued(&name, &out.verb) && i + 1 < argv.len() {
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
        self.options.iter().rev().find(|(n, _)| n == name).and_then(|(_, v)| v.as_deref())
    }

    fn all(&self, name: &str) -> Vec<&str> {
        self.options.iter().filter(|(n, _)| n == name).filter_map(|(_, v)| v.as_deref()).collect()
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
pub(crate) const KNOWN_FLAGS: &[(&str, usize, &[&str])] = &[
    ("new", 0, &["--dir", "--id", "--slug", "--priority", "--points", "--parent", "--requires", "--spec", "--review-url", "--body", "--body-file", "--empty"]),
    ("edit", 0, prose::EDIT_FLAGS),
    ("sync", 0, &["--dir"]),
    ("mv", 0, &["--dir", "--resolution", "--review-url"]),
    ("start", 0, &["--dir"]),
    ("review", 0, &["--dir", "--review-url"]),
    ("done", 0, &["--dir", "--resolution"]),
    ("set", 0, &["--dir", "--auto", "--priority", "--points", "--parent", "--spec", "--review-url", "--title", "--slug", "--field", "--unset"]),
    ("dep", 0, &["--dir", "--add", "--remove"]),
    ("label", 0, &["--dir", "--add", "--remove"]),
    ("list", 0, tables::LIST_FLAGS),
    // Named once rather than repeated: `tree` is an alias, so a flag either verb accepted
    // alone would be a flag the other silently refused.
    ("tree", 0, tables::LIST_FLAGS),
    ("show", 0, &["--dir", "--json"]),
    ("path", 0, &["--dir"]),
    ("which", 0, &["--dir", "--ids"]),
    ("check", 0, &["--dir"]),
    ("html", 0, &["--dir", "--out", "--cmd"]),
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

/// Everything wrong with the *shape* of the invocation, as opposed to what it asks for.
///
/// Kept separate because it exits 2, not 1. That is argparse's convention and it is a
/// real distinction: a script can tell "you called me wrong" from "the thing you asked
/// for failed".
fn usage_error(args: &Args) -> Option<String> {
    // Answered specifically rather than as a typo: the Python engine had `update`, and
    // someone with the habit deserves to be told what replaced it. The binary does not
    // replace itself — whatever installed it owns the file, and a self-updater fighting
    // a package manager is worse than having none.
    // Renamed to sit beside `deps --requires`, which already means "what this needs".
    // A bare "unrecognized argument" would be correct and useless: the flag existed, the
    // spelling moved, and only this message can say so.
    if args.has("--depends") {
        return Some(format!(
            "`--depends` is now `--requires` (it reads with `trck deps --requires`, which \
             shows the same edges). Try: trck {} --requires <ids>",
            args.verb
        ));
    }
    if args.verb == "update" {
        return Some(
            "`update` is gone: trck is a binary now, upgraded however you installed it \
             (your package manager, or re-running the install script). `trck version` \
             reports what you have."
                .to_string(),
        );
    }
    if !args.verb.is_empty() && !tables::VERBS.contains(&args.verb.as_str()) {
        return Some(format!("unknown verb `{}`", args.verb));
    }
    // A flag that is accepted and ignored returns human text with exit 0, and a caller piping
    // into `jq` finds out far from the cause. So a verb with no `--json` refuses it rather
    // than dropping it; `JSON_VERBS` is the list of the ones that answer.
    if args.has("--json") && !tables::JSON_VERBS.contains(&args.verb.as_str()) {
        return Some(format!("{}: --json is not implemented in this engine yet", args.verb));
    }
    if let Some(n) = unrecognized_flag(args) {
        return Some(format!("{}: unrecognized argument {n}", args.verb));
    }
    if let Some((_, want, what)) = tables::MIN_POSITIONAL.iter().find(|(verb, ..)| *verb == args.verb)
        && args.positional.len() < *want
    {
        return Some(format!("{}: missing {what}", args.verb));
    }
    if let Some((_, opt)) = tables::REQUIRED_OPTS.iter().find(|(verb, _)| *verb == args.verb)
        && args.opt(opt).is_none()
    {
        return Some(format!("{}: the following arguments are required: {opt}", args.verb));
    }
    None
}

fn usage() -> String {
    format!(
        "trck {} — deterministic in-repo issue tracker\n\
         \n\
         One binary, no runtime and no dependencies. The tracker is plain files in your\n\
         repository: a markdown body per issue, plus a generated index and summary.\n\
         What the verbs do is specified by conformance/, which runs against this binary.\n\
         \n\
         Verbs: {}\n",
        env!("CARGO_PKG_VERSION"),
        tables::VERBS.join(", ")
    )
}

/// Resolve the tracker and load it, applying the format guard.
/// Whether a write to stdout failed because nobody is reading any more.
///
/// `trck list | head` closes the pipe while the engine still has output. That is the shell
/// working as designed, not a failure to report, so it ends the process quietly and
/// successfully. It has to be handled rather than ignored: `println!` unwraps the write and
/// would panic, which is precisely what this crate's denied `panic`/`unwrap`/`expect` lints
/// exist to prevent — and a standard-library macro walks underneath them. The usual Unix
/// answer, restoring the default `SIGPIPE` disposition so the process dies by signal, needs
/// a raw `signal()` call, and `unsafe` is forbidden here.
fn is_closed_pipe(e: &std::io::Error) -> bool {
    e.kind() == std::io::ErrorKind::BrokenPipe
}

/// Write to stdout and flush, reporting a closed reader rather than panicking on it.
///
/// Flushing here rather than leaving it to process teardown is the point: `Stdout` is
/// line-buffered, so without an explicit flush the failing write can happen after `main`
/// has returned, where there is no longer anything that can decide what it means.
fn emit(text: &str) -> Result<(), std::io::Error> {
    use std::io::Write as _;
    let stdout = std::io::stdout();
    let mut lock = stdout.lock();
    lock.write_all(text.as_bytes())?;
    lock.flush()
}

/// `emit`, resolved to the status the process should exit with.
fn emit_or_status(text: &str, ok: std::process::ExitCode) -> std::process::ExitCode {
    match emit(text) {
        Ok(()) => ok,
        Err(ref e) if is_closed_pipe(e) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: writing output: {e}");
            std::process::ExitCode::FAILURE
        },
    }
}

/// Entry point. Returns the process status; everything user-facing goes to stdout on
/// success and stderr on failure, matching the Python engine's `die`.
pub(crate) fn main() -> std::process::ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    if argv.is_empty() || argv.iter().any(|a| a == "-h" || a == "--help") {
        // `trck <verb> --help` is a question about the verb. Falling back to the program's
        // own help when the verb is unknown beats an error: someone reaching for help is
        // already telling you they do not know what to type.
        let text = argv.first().filter(|a| !a.starts_with('-')).and_then(|verb| help::for_verb(verb)).unwrap_or_else(usage);
        return emit_or_status(&text, std::process::ExitCode::SUCCESS);
    }
    if let Some(msg) = usage_error(&parse_args(&argv)) {
        eprintln!("error: {msg}");
        return std::process::ExitCode::from(2);
    }
    match dispatch(&argv) {
        Ok(out) if out.is_empty() => std::process::ExitCode::SUCCESS,
        Ok(out) => emit_or_status(&(out + "\n"), std::process::ExitCode::SUCCESS),
        Err(msg) if msg.is_empty() => std::process::ExitCode::FAILURE,
        // A message that already labels itself is printed verbatim. The merge drivers are
        // the case: git shows their stderr to the user as-is, so the diagnostic is written
        // as a whole report — a headline, the conflicting rows, and what to do next — and
        // prefixing `error:` onto its first line would read as though only that line were
        // the error.
        Err(msg) if msg.starts_with("trck: ") => {
            eprintln!("{msg}");
            std::process::ExitCode::FAILURE
        },
        Err(msg) => {
            eprintln!("error: {msg}");
            std::process::ExitCode::FAILURE
        },
    }
}

#[cfg(test)]
mod tests {
    // Tests assert; that is their job. The crate denies unwrap/expect/panic because a
    // malformed tracker must produce a diagnostic rather than a stack trace, but a test
    // that cannot panic cannot fail.
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    /// The preamble is the first thing a user reads, and it is the one piece of shipped text
    /// with no fixture behind it. It said the read verbs did not work yet for as long as they
    /// have worked. The standing arrangement is that progress is measured by `conformance/`
    /// and never described in prose, so the help must not make a claim that can go stale.
    #[test]
    fn the_help_does_not_narrate_the_state_of_the_port() {
        let text = usage();
        for stale in ["in progress", "do not yet", "not yet", "so far", "for now", "currently"] {
            assert!(!text.to_lowercase().contains(stale), "help claims a state that will go stale ({stale:?}): {text}");
        }
        assert!(text.contains(env!("CARGO_PKG_VERSION")), "{text}");
        for verb in ["new", "list", "check", "html"] {
            assert!(text.contains(verb), "help omits `{verb}`: {text}");
        }
    }

    /// The same word means the same relationship from either end — what an issue needs —
    /// so both verbs spell it `--requires`. They differ in arity, and that is the whole
    /// hazard: with a single global table, `deps --requires --json` swallowed `--json` as
    /// the value of `--requires` and emitted human text to a caller expecting JSON.
    #[test]
    fn requires_takes_a_value_for_new_and_none_for_deps() {
        let a = args(&["new", "Title", "--requires", "aaaaaaa,bbbbbbb"]);
        assert_eq!(a.opt("--requires"), Some("aaaaaaa,bbbbbbb"));

        let d = args(&["deps", "bbbbbbb", "--requires", "--json"]);
        assert!(d.has("--requires"), "deps lost its filter");
        assert!(d.has("--json"), "--json was eaten as a value");
        assert_eq!(d.opt("--requires"), None, "deps' filter took a value");
    }

    /// A rename is the one case where "unrecognized argument" is correct and useless: the
    /// flag existed, its spelling moved, and only a specific message can say so.
    #[test]
    fn the_old_spelling_names_the_new_one() {
        let msg = usage_error(&args(&["new", "Title", "--depends", "aaaaaaa"])).expect("refused");
        assert!(msg.contains("--requires"), "{msg}");
        assert!(msg.contains("--depends"), "{msg}");
    }

    fn args(argv: &[&str]) -> Args {
        parse_args(&argv.iter().map(|s| (*s).to_string()).collect::<Vec<_>>())
    }

    #[test]
    fn a_verb_with_positionals_and_options() {
        let a = args(&["new", "Fix the parser", "--priority", "high", "--id", "aaaaaaa"]);
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
        assert_eq!(args(&["--dir", "issues", "new", "T"]).opt("--dir"), Some("issues"));
        assert_eq!(args(&["new", "T", "--dir", "issues"]).opt("--dir"), Some("issues"));
        assert_eq!(args(&["--dir", "issues", "new", "T"]).verb, "new");
    }

    #[test]
    fn an_unrecognised_flag_is_refused_rather_than_ignored() {
        // `list --stauts done` would otherwise list everything and read as a filter.
        let bad = parse_args(&["list".into(), "--stauts".into(), "done".into()]);
        assert!(usage_error(&bad).is_some_and(|m| m.contains("unrecognized argument --stauts")), "a typo'd flag must be refused, not dropped");
        // A flag the verb does accept passes.
        assert_eq!(usage_error(&parse_args(&["list".into(), "--all".into()])), None);
    }

    #[test]
    fn json_is_honoured_by_the_read_verbs_and_refused_elsewhere() {
        // Accepting the flag and printing human output with exit 0 is the one failure a
        // caller cannot detect — `trck list --json | jq` would break far from its cause.
        // So every verb either implements it or says it does not.
        for argv in [
            vec!["list".to_string(), "--json".to_string()],
            vec!["tree".to_string(), "--json".to_string()],
            vec!["show".to_string(), "aaaaaaa".to_string(), "--json".to_string()],
            vec!["ready".to_string(), "--json".to_string()],
            vec!["next".to_string(), "--json".to_string()],
            vec!["deps".to_string(), "--json".to_string()],
        ] {
            let verb = argv[0].clone();
            assert_eq!(usage_error(&parse_args(&argv)), None, "{verb} --json is implemented and must parse");
        }
        // A verb with no --json still refuses it rather than ignoring it.
        let msg = usage_error(&parse_args(&["summary".to_string(), "--json".to_string()]));
        assert!(msg.as_ref().is_some_and(|m| m.contains("--json")), "summary --json must be refused, got {msg:?}");
        assert_eq!(usage_error(&parse_args(&["list".into()])), None);
    }

    #[test]
    fn a_missing_required_option_is_a_usage_error() {
        assert!(usage_error(&parse_args(&["changelog".into()])).is_some_and(|m| m.contains("required: --since")));
        assert_eq!(usage_error(&parse_args(&["changelog".into(), "--since".into(), "2026-01-01".into()])), None);
    }

    #[test]
    fn a_missing_positional_is_a_usage_error_not_an_operation_that_fails() {
        // It exits 2, like argparse: a script can tell "you called me wrong" from
        // "what you asked for failed".
        assert!(usage_error(&parse_args(&["show".into()])).is_some_and(|m| m.contains("missing")));
        assert!(usage_error(&parse_args(&["mv".into(), "abc".into()])).is_some_and(|m| m.contains("missing")));
    }

    #[test]
    fn an_unimplemented_verb_says_so_rather_than_guessing() {
        // Saying so beats producing something approximate: a half-implemented verb is what
        // would turn the conformance pass rate into a lie. This test named `list`, then
        // `deps`, then `changelog`, then `init` as each landed — the churn was the point,
        // it tracked the frontier. `init` was the last of them, so what it guards now is
        // `repo`, whose subcommand list is the one that can still grow.
        let err = dispatch(&["repo".to_string(), "nonesuch".to_string()]).expect_err("not implemented");
        assert!(err.contains("not implemented"), "{err}");
        let unknown = usage_error(&parse_args(&["nonesuch".to_string()]));
        assert!(unknown.is_some_and(|m| m.contains("unknown verb")));
    }

    /// The catch-all that told users a verb was unported is now unreachable from the top
    /// level. Asserting that here means the day someone adds a verb to `VERBS` without
    /// wiring it, this test — not a user — is what finds out.
    #[test]
    fn every_advertised_verb_is_wired_to_something() {
        for verb in tables::VERBS {
            // Dispatching would run them; the frontier is a property of the match arms, so
            // read it off the message the catch-all would produce instead.
            let orphan = format!("`{verb}` is not implemented yet in the Rust engine");
            assert!(!usage().contains(&orphan), "the help advertises an unported verb: {verb}");
        }
        assert!(tables::VERBS.contains(&"init"), "init dropped out of the verb list");
    }
}

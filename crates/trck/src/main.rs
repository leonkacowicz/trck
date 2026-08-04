//! The Rust trck engine.
//!
//! Nothing is implemented yet, and that is deliberate. The conformance suite
//! (`conformance/`) is written against a binary, not against a library, so this crate
//! can exist and be measured from the first commit: CI runs the fixtures against it and
//! reports how many pass. Today that number is zero. It goes up as the port lands.
//!
//! The inversion matters. The usual rewrite is assessed at the end, by reading code and
//! hoping; this one is assessed continuously, by a suite that already encodes what the
//! Python engine does. See `conformance/README.md` for the fixture format and
//! `issues/` (`#sp2rwzx`) for the port's plan.

use std::process::ExitCode;

/// Verbs the Python engine has. Listed so `--help` can be honest about what this
/// binary is *for* while it is still empty, and so the port has a checklist that
/// lives next to the code rather than only in the tracker.
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

fn usage() -> String {
    format!(
        "trck {} (Rust) — deterministic in-repo issue tracker\n\
         \n\
         Nothing is implemented yet. This binary exists so the conformance suite can\n\
         measure the port from the first commit; see conformance/README.md.\n\
         \n\
         Planned verbs: {}\n",
        env!("CARGO_PKG_VERSION"),
        VERBS.join(", ")
    )
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.iter().any(|a| a == "-h" || a == "--help") || args.is_empty() {
        print!("{}", usage());
        return ExitCode::SUCCESS;
    }

    // Skip the global options the real CLI takes, so the diagnostic names the verb
    // rather than `--dir`. Cheap, and it makes the conformance output readable while
    // every fixture is still failing.
    let mut rest = args.iter();
    let mut verb: Option<&String> = None;
    while let Some(a) = rest.next() {
        match a.as_str() {
            "--dir" => {
                rest.next();
            }
            _ if a.starts_with('-') => {}
            _ => {
                verb = Some(a);
                break;
            }
        }
    }

    match verb {
        Some(v) if VERBS.contains(&v.as_str()) => {
            eprintln!("error: `{v}` is not implemented yet in the Rust engine");
        }
        Some(v) => eprintln!("error: unknown verb `{v}`"),
        None => eprintln!("error: no verb given"),
    }
    ExitCode::FAILURE
}

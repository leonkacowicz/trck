//! The Rust trck engine.
//!
//! The port is in progress and measured rather than described: the conformance suite
//! (`conformance/`) runs against a *binary*, so CI reports how many fixtures pass on
//! every commit. That inverts the usual rewrite failure mode, where correctness is
//! assessed at the end by reading code and hoping.
//!
//! The inversion matters. The usual rewrite is assessed at the end, by reading code and
//! hoping; this one is assessed continuously, by a suite that already encodes what the
//! Python engine does. See `conformance/README.md` for the fixture format and
//! `issues/` (`#sp2rwzx`) for the port's plan.

// The model was built before the verbs that read it, so parts of it are still
// unreferenced. `expect` rather than `allow`: once the port has wired everything up, the
// expectation goes unfulfilled and the compiler says so, which is a better reminder to
// delete this than a comment would be.
#![expect(dead_code, reason = "the model lands before the verbs that read it")]

mod cli;
mod config;
mod diff;
mod discovery;
mod graph;
mod gutter;
mod html;
mod id;
mod index;
mod issue;
mod json;
mod merge;
mod query;
mod render;
mod repo;
mod summary;
mod validate;
mod verbs;

fn main() -> std::process::ExitCode {
    cli::main()
}

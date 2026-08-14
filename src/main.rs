//! The trck engine.
//!
//! Correctness here is measured rather than described. The conformance suite
//! (`conformance/`) runs against a *binary* — it execs `TRCK_BIN` and never imports an
//! engine — so CI reports on every commit how many fixtures this one satisfies. That
//! inverts the usual rewrite failure mode, where a port is assessed at the end by reading
//! code and hoping; this one was assessed continuously, from its first commit, against a
//! suite that already encoded what the engine it replaced did.
//!
//! It stays useful past the port: the fixtures are the specification, so anything a user
//! or a downstream tool would notice belongs there rather than in a unit test. See
//! `conformance/README.md` for the fixture format and `issues/` (`#sp2rwzx`) for the
//! port's plan.

// The model was built before the verbs that read it, so parts of it are still
// unreferenced. `expect` rather than `allow`: once the port has wired everything up, the
// expectation goes unfulfilled and the compiler says so, which is a better reminder to
// delete this than a comment would be.
#![expect(dead_code, reason = "the model lands before the verbs that read it")]

mod cli;
mod config;
mod diff;
mod discovery;
mod git;
mod graph;
mod gutter;
mod help;
mod html;
mod id;
mod index;
mod init;
mod issue;
mod json;
mod merge;
mod query;
mod render;
mod repo;
mod summary;
#[cfg(test)]
mod test_graph;
mod validate;
mod verbs;

fn main() -> std::process::ExitCode {
    cli::main()
}

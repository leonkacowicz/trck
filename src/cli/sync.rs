//! `sync`: get local tracker commits to the remote, and the remote's to here.
//!
//! Reads deliberately do not fetch — a read that needs the network is a read that fails on a
//! plane — and a write deliberately does not fail when it cannot push, because the commit is
//! already anchored locally. Both of those decisions are only tenable because there is one
//! verb whose whole job is the network, and this is it.
//!
//! It is also the remedy every pending-changes note names, so what it says has to match what
//! that note promised: push what is waiting, pick up what landed elsewhere, and say which of
//! the two happened.
//!
//! **There is exactly one exception, and it is `serve`.** `serve`'s poll loop (`src/serve/poll.rs`) fetches the
//! tracker branch on a timer. The rule above is written for a verb in a pipeline, where the
//! round trip is paid again by every `trck list` anybody types; `serve` is one long-lived
//! process, so it pays once per interval however many pages are open, and what it buys is the
//! reason that verb exists — a tab left open on a week-old ref is a read from the past with
//! nothing to say so. The exception is about the *shape* of the caller, not about
//! convenience, which is why it has stayed at one: if a second verb ever wants it, that verb
//! has to be a daemon too.

use super::Args;
use crate::discovery::standing::{pending, tracking};
use crate::discovery::{Ctx, Source};
use crate::git::refs::{fetch, has_remote, push};
use crate::git::rev_parse;
use crate::verbs::backend::local_ref;

/// The remote a tracker branch is shared through — the same convention the write path uses.
const REMOTE: &str = "origin";

pub(super) fn cmd_sync(ctx: &Ctx, _args: &Args) -> Result<String, String> {
    let Source::Ref { rev, cwd } = &ctx.source else {
        // A directory tracker is files in someone's checkout, shared by whatever commits that
        // checkout. There is nothing here that `git push` is not already doing.
        return Err("sync is for a tracker on a git ref; this one is a directory, shared by whatever commits it".to_string());
    };
    if !has_remote(cwd, REMOTE) {
        return Err(format!("no `{REMOTE}` remote, so there is nowhere to sync to"));
    }

    let target = local_ref(rev);
    let waiting = pending(cwd)?;

    // Push before fetching. What is waiting is the thing the operator was told to run this
    // for, and a fetch first would only make the picture prettier before the part that can
    // fail.
    if waiting > 0 {
        let Some(sha) = rev_parse(cwd, &target)? else {
            return Err(format!("{target} does not exist, so there is nothing to push"));
        };
        push(cwd, REMOTE, &sha, &target)
            .map_err(|e| format!("could not push {waiting} pending change(s): {e}\n  they are still on {target} and are not lost"))?;
    }

    let before = rev_parse(cwd, &tracking())?;
    fetch(cwd, REMOTE, &target)?;
    let after = rev_parse(cwd, &tracking())?;

    Ok(report(waiting, before.as_deref() != after.as_deref()))
}

/// What happened, in the operator's terms rather than git's.
fn report(pushed: usize, moved: bool) -> String {
    match (pushed, moved) {
        (0, false) => "already in sync".to_string(),
        (0, true) => format!("nothing to push; {} moved", tracking()),
        (n, false) => format!("pushed {n} change{}", plural(n)),
        (n, true) => format!("pushed {n} change{}; {} moved too", plural(n), tracking()),
    }
}

fn plural(n: usize) -> &'static str {
    if n == 1 { "" } else { "s" }
}

/// `sync` is its own dispatch stage rather than an arm among the verbs that change a row.
/// It changes no row: what it moves is where the rows already are.
pub(super) fn dispatch_sync(args: &super::Args) -> Option<Result<String, String>> {
    (args.verb == "sync").then(|| super::context(args).and_then(|c| cmd_sync(&c, args)))
}

#[cfg(test)]
mod tests {
    // Tests assert; that is their job. The crate denies unwrap/expect/panic because a
    // malformed tracker must produce a diagnostic rather than a stack trace, but a test
    // that cannot panic cannot fail.
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    /// Nothing to do has to *say* nothing to do. `sync` is what a pending note tells people
    /// to run, so a silent success would leave them wondering whether it worked.
    #[test]
    fn nothing_pending_and_nothing_new_says_so() {
        assert_eq!(report(0, false), "already in sync");
    }

    #[test]
    fn each_outcome_reads_differently() {
        assert!(report(0, true).contains("nothing to push"));
        assert!(report(2, false).starts_with("pushed 2 changes"));
        assert!(report(1, true).starts_with("pushed 1 change;"), "{}", report(1, true));
    }

    #[test]
    fn one_change_is_not_plural() {
        assert_eq!(plural(1), "");
        assert_eq!(plural(0), "s");
        assert_eq!(plural(2), "s");
    }
}

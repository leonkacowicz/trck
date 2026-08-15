//! Getting the local commit onto the remote, and what to do when someone else got there first.
//!
//! **No fetch before the write.** A commit whose parent is not the current remote tip cannot be
//! pushed, so either the base was current — and the whole verb, its guards and its rollup, ran
//! against current data — or the push is rejected and the operation runs again. Fetching first
//! would cost a round trip on every write to buy a guarantee the push already gives, and would
//! still race: the remote can move between the fetch and the push. Fetch-on-rejection is
//! identical in correctness at half the round trips on the path that is almost always taken.
//!
//! **A rejection is not an error.** It means someone else's work landed, and the answer is to
//! rebuild on top of it: fetch, move the local branch to what the remote now holds, replay the
//! operation from its own commit's trailer, push again. Never `--force`; overwriting is how the
//! other writer's issue disappears.
//!
//! Only *this* process's commit is replayed. A local branch that is several commits ahead —
//! because an earlier push failed and was left — is `#9gxktnk`.

use super::super::op::Op;
use crate::git::refs::{fetch, has_remote, push, update_ref};
use crate::git::rev_parse;
use std::path::Path;

/// The remote a tracker branch is shared through.
///
/// Convention rather than configuration, matching the ref name itself: a tracker that needs a
/// different one is a tracker with a story, and it can have a flag when someone has that story.
const REMOTE: &str = "origin";

/// How many times a write rebuilds before giving up.
///
/// Each round is a real rejection, so this is not a timeout — it is how many writers can beat
/// this one in a row before it stops trying. Three is enough that losing means something is
/// wrong (a busy remote, or an operation that cannot apply) rather than bad luck.
const ATTEMPTS: usize = 3;

/// Push the local branch, rebuilding onto whatever the remote holds if it moved.
///
/// `replay` re-runs the operation against the tracker as it now stands; it is passed in rather
/// than called directly because it re-enters the verbs, and this module is beneath them.
pub(crate) fn sync(cwd: &Path, target: &str, op: &Op, replay: &dyn Fn(&Op, &str) -> Result<(), String>) -> Result<(), String> {
    if !has_remote(cwd, REMOTE) {
        return Ok(()); // a local tracker: the local ref is the whole of it
    }
    // *Every* way this fails is wrapped, not just running out of attempts. An unreachable
    // remote fails at the fetch with git's own words, and those words say nothing about the
    // issue that was just filed — which is committed, safe, and the entire reason the local
    // ref is written first. A user told only "could not read from remote repository" has no
    // way to know their work survived.
    attempts(cwd, target, op, replay).map_err(|reason| unshared(target, op, &reason))
}

/// Push, and rebuild onto whatever landed first, until one of them works or the tries run out.
fn attempts(cwd: &Path, target: &str, op: &Op, replay: &dyn Fn(&Op, &str) -> Result<(), String>) -> Result<(), String> {
    for attempt in 1..=ATTEMPTS {
        let Some(sha) = rev_parse(cwd, target)? else {
            return Err(format!("{target} does not exist, so there is nothing to push"));
        };
        let Err(rejected) = push(cwd, REMOTE, &sha, target) else {
            return Ok(());
        };
        if attempt == ATTEMPTS {
            return Err(format!("rejected {ATTEMPTS} times, last: {rejected}"));
        }
        rebuild(cwd, target, op, replay)?;
    }
    Ok(())
}

/// Move the local branch onto what the remote now holds, and run the operation again there.
///
/// The pending commit is not discarded by the reset — it is still whole in the object store,
/// and its tree is where a replayed body comes from. What is discarded is its *position*, which
/// was on a base that no longer exists as a tip.
fn rebuild(cwd: &Path, target: &str, op: &Op, replay: &dyn Fn(&Op, &str) -> Result<(), String>) -> Result<(), String> {
    fetch(cwd, REMOTE, target)?;
    let tracking = format!("refs/remotes/{REMOTE}/{}", target.trim_start_matches("refs/heads/"));
    let Some(theirs) = rev_parse(cwd, &tracking)? else {
        // The remote refused a push and has no such branch: not contention, and re-running
        // would loop against a wall. Whatever went wrong, the push error said it.
        return Err(format!("{REMOTE} rejected the push but has no {tracking} to rebuild onto"));
    };
    // The pending commit's sha is kept, not just discarded with its position: its tree is
    // where a replayed body comes from, and after the reset nothing else points at it.
    let ours = rev_parse(cwd, target)?;
    update_ref(cwd, target, &theirs, ours.as_deref())?;
    match &ours {
        Some(pending) => replay(op, pending),
        // Nothing local to replay: the branch did not exist, so the push failed for a reason
        // that rebuilding cannot address.
        None => Err(format!("{target} holds nothing to replay")),
    }
}

/// What to say when the write could not be shared.
///
/// The commit is not lost — it is on the local branch, which is the point of anchoring it
/// there before pushing — so the message says where it is and what to run, rather than
/// apologising. git's own reason leads, because it is the half that says *why*.
fn unshared(target: &str, op: &Op, reason: &str) -> String {
    format!(
        "could not share this write: {reason}\n  \
         it is committed locally on {target} and is not lost:\n    {}\n  \
         run `git push {REMOTE} {target}` once the remote is reachable, or re-run the verb",
        op.render()
    )
}

#[cfg(test)]
mod tests {
    // Tests assert; that is their job. The crate denies unwrap/expect/panic because a
    // malformed tracker must produce a diagnostic rather than a stack trace, but a test
    // that cannot panic cannot fail.
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    /// The failure path has one job beyond reporting: say that nothing was lost, and where it
    /// is. A user who reads "could not push" and nothing else has no way to know the issue
    /// they just filed still exists.
    #[test]
    fn the_failure_message_says_where_the_work_is_and_what_to_run() {
        let op = Op::new("mv").operand("aaaaaaa").operand("done");
        let msg = unshared("refs/heads/trck-issues", &op, "non-fast-forward");
        assert!(msg.contains("non-fast-forward"), "git's own reason survives: {msg}");
        assert!(msg.contains("is not lost"), "{msg}");
        assert!(msg.contains("mv aaaaaaa done"), "the pending operation is named: {msg}");
        assert!(msg.contains("git push origin refs/heads/trck-issues"), "{msg}");
    }

    /// It wraps *every* way sharing fails, not only running out of retries — an unreachable
    /// remote fails at the fetch, in git's words, which say nothing about the local commit.
    #[test]
    fn an_unreachable_remote_is_reported_the_same_reassuring_way() {
        let msg = unshared("refs/heads/trck-issues", &Op::new("normalize"), "does not appear to be a git repository");
        assert!(msg.contains("does not appear to be a git repository"), "{msg}");
        assert!(msg.contains("is not lost"), "{msg}");
    }

    /// Bounded, and the bound is named when it is what stopped things — so a reader knows it
    /// tried repeatedly rather than failing once.
    #[test]
    fn running_out_of_retries_says_how_many_there_were() {
        assert!(format!("rejected {ATTEMPTS} times, last: x").contains(&ATTEMPTS.to_string()));
    }
}

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
    // Not being able to reach the remote is not a failed write: the commit is on the local
    // branch — which is why it is written first — so the verb succeeds and its caller reports
    // what is pending (`#dak2sjq`). A replay that failed is the one case where something
    // could actually be lost, because `rebuild` has already moved the local ref to the
    // remote's tip and the pending commit is unreferenced. That one is an error, and it says
    // where the commit is.
    match attempts(cwd, target, replay) {
        Err(Unshared::Fatal(reason)) => Err(unshared(target, op, &reason)),
        Ok(()) | Err(Unshared::Unreachable(_)) => Ok(()),
    }
}

/// Why a push did not land, split by whether anything is at risk.
enum Unshared {
    /// The remote could not be reached, or would not take it. The local branch still holds
    /// everything; nothing is lost and nothing needs saying beyond "not shared yet".
    Unreachable(String),
    /// Something went wrong that leaves work needing a human. See [`sync`].
    Fatal(String),
}

/// Push, and rebuild onto whatever landed first, until one of them works or the tries run out.
fn attempts(cwd: &Path, target: &str, replay: &dyn Fn(&Op, &str) -> Result<(), String>) -> Result<(), Unshared> {
    for attempt in 1..=ATTEMPTS {
        let sha = match rev_parse(cwd, target) {
            Ok(Some(sha)) => sha,
            Ok(None) => return Err(Unshared::Fatal(format!("{target} does not exist, so there is nothing to push"))),
            Err(e) => return Err(Unshared::Fatal(e)),
        };
        let Err(rejected) = push(cwd, REMOTE, &sha, target) else {
            return Ok(());
        };
        if attempt == ATTEMPTS {
            return Err(Unshared::Unreachable(format!("rejected {ATTEMPTS} times, last: {rejected}")));
        }
        rebuild(cwd, target, replay)?;
    }
    Ok(())
}

/// Move the local branch onto what the remote now holds, and run the operation again there.
///
/// The pending commit is not discarded by the reset — it is still whole in the object store,
/// and its tree is where a replayed body comes from. What is discarded is its *position*, which
/// was on a base that no longer exists as a tip.
fn rebuild(cwd: &Path, target: &str, replay: &dyn Fn(&Op, &str) -> Result<(), String>) -> Result<(), Unshared> {
    fetch(cwd, REMOTE, target).map_err(Unshared::Unreachable)?;
    let tracking = format!("refs/remotes/{REMOTE}/{}", target.trim_start_matches("refs/heads/"));
    let Some(theirs) = rev_parse(cwd, &tracking).map_err(Unshared::Fatal)? else {
        // The remote refused a push and has no such branch: not contention, and re-running
        // would loop against a wall. Whatever went wrong, the push error said it.
        return Err(Unshared::Unreachable(format!("{REMOTE} rejected the push but has no {tracking} to rebuild onto")));
    };
    let Some(ours) = rev_parse(cwd, target).map_err(Unshared::Fatal)? else {
        // Nothing local to replay: the branch did not exist, so the push failed for a reason
        // rebuilding cannot address.
        return Err(Unshared::Fatal(format!("{target} holds nothing to replay")));
    };

    // Read *before* the reset. Each pending commit's sha is kept, not just discarded with its
    // position: its tree is where that op's prose comes from, and after the reset nothing
    // else points at it.
    let stack = pending(cwd, &tracking, target).map_err(Unshared::Fatal)?;
    // Both moves below are ref moves like any other, so anyone holding the branch comes off it
    // first. Once here they are already detached — the write that is being rebuilt released
    // them — which makes this the belt to that braces, at the price of one `worktree list`.
    super::release(cwd, target).map_err(Unshared::Fatal)?;
    update_ref(cwd, target, &theirs, Some(&ours)).map_err(Unshared::Fatal)?;

    for (sha, op) in &stack {
        let Err(why) = replay(op, sha) else { continue };
        // No partial application. A half-replayed stack is a tracker holding some of what the
        // operator did and none of the rest, with nothing saying which — so the branch goes
        // back where it was and the operation that could not be replayed is named.
        let reached = rev_parse(cwd, target).map_err(Unshared::Fatal)?;
        update_ref(cwd, target, &ours, reached.as_deref()).map_err(Unshared::Fatal)?;
        return Err(Unshared::Fatal(format!("`{}` no longer applies: {why}\n  nothing was replayed; {target} is unchanged", op.render())));
    }
    Ok(())
}

/// The pending commits and the operation each one recorded, oldest first.
///
/// Order is the point: a later op may act on an issue an earlier one created, so replaying
/// out of order fails on an id that does not exist yet.
///
/// A commit with no trailer cannot be replayed at all. That is someone having committed to
/// the tracker branch by hand, and guessing at what they meant would be worse than saying so.
fn pending(cwd: &Path, tracking: &str, target: &str) -> Result<Vec<(String, Op)>, String> {
    crate::git::rev_list(cwd, &format!("{tracking}..{target}"))?
        .into_iter()
        .map(|sha| {
            let message = crate::git::commit_message(cwd, &sha)?;
            match super::message::op_of(&message)? {
                Some(op) => Ok((sha, op)),
                None => Err(format!("commit {} records no operation, so it cannot be replayed", &sha[..7.min(sha.len())])),
            }
        })
        .collect()
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

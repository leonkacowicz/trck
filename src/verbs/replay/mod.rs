//! Running an operation again, against a tracker that has moved underneath it.
//!
//! This is what a rejected push does instead of forcing. The [`Op`] recovered from the pending
//! commit's trailer is turned back into the verb call that produced it, and that call runs
//! against the tracker as it now stands — so the guards run again, the rollup is derived from
//! the *new* rows, and the result is a commit that could have been made in the first place had
//! the other writer gone first.
//!
//! **Re-running the verb is the point.** A textual merge of `index.jsonl` would put both rows
//! in the file and call it done, which is wrong the moment a parent's status or points depend
//! on what the other writer changed. Derivation cannot be merged; it has to be redone.
//!
//! **Prose comes from the pending commit's tree, not from the op.** An op records intent, and a
//! body is content — duplicating it into the trailer would give the same bytes two homes that
//! can disagree, and cost every `new` commit its whole body in the message. The commit that is
//! being replayed still holds it, which is what anchoring on the local ref before pushing buys.

mod opts;

use super::{Edit, MvOpts, NewOpts, Op, body_rel_path, cmd_dep, cmd_label, cmd_mv, cmd_new, cmd_set, commit, load_rows, resolve_ref};
use crate::discovery::{Ctx, ITEMS_DIR, Source};
use crate::graph::Graph;
use opts::{set_opts, values};

/// Run `op` again against the tracker as it now stands.
///
/// `pending` is the commit the op was originally committed as — the source of any prose it
/// wrote, and nothing else.
pub(crate) fn replay(ctx: &Ctx, op: &Op, pending: &str) -> Result<(), String> {
    match op.verb.as_str() {
        // `new`'s first operand is a title, not an id, which is why it is settled apart from
        // the verbs below that all name an issue.
        "new" => replay_new(ctx, op, pending),
        // The two that act on the whole tracker. `summary` regenerates from the index it is
        // given, so replaying it against the new one is simply running it.
        "summary" => return super::write_summary(ctx, &Graph::new(load_rows(ctx)?)),
        "normalize" => commit(ctx, load_rows(ctx)?, Vec::new(), op).map(|rows| format!("{} issues", rows.len())),
        _ => on_issue(ctx, op, pending),
    }
    .map(|_| ())
}

/// The verbs that name the issue they act on, with that name resolved once for all of them.
fn on_issue(ctx: &Ctx, op: &Op, pending: &str) -> Result<String, String> {
    let id = op.operands.first().map(String::as_str).ok_or_else(|| format!("cannot replay `{}`: it names no issue", op.render()))?;
    match op.verb.as_str() {
        "edit" => replay_edit(ctx, id, pending),
        "mv" => replay_mv(ctx, op, id),
        "set" => cmd_set(ctx, id, &set_opts(op)),
        "label" => cmd_label(ctx, id, &values(op, "--add"), &values(op, "--remove")),
        "dep" => cmd_dep(ctx, id, op.flag_value("--add"), op.flag_value("--remove")),
        other => Err(format!("cannot replay `{other}`: no such operation")),
    }
}

/// `mv`, which is the one verb whose second operand carries meaning.
fn replay_mv(ctx: &Ctx, op: &Op, target: &str) -> Result<String, String> {
    let status = op.operands.get(1).ok_or_else(|| format!("cannot replay `{}`: it names no status", op.render()))?;
    let opts = MvOpts { status, resolution: op.flag_value("--resolution"), review_url: op.flag_value("--review-url") };
    cmd_mv(ctx, target, &opts)
}

/// `new`, with its prose read back out of the commit it was first written into.
fn replay_new(ctx: &Ctx, op: &Op, pending: &str) -> Result<String, String> {
    let title = op.operands.first().ok_or_else(|| format!("cannot replay `{}`: it names no title", op.render()))?;
    let opts = NewOpts {
        title: title.clone(),
        id: op.flag_value("--id").map(str::to_string),
        slug: op.flag_value("--slug").map(str::to_string),
        priority: op.flag_value("--priority").map(str::to_string),
        points: op.flag_value("--points").and_then(|p| p.parse().ok()),
        parent: op.flag_value("--parent").map(str::to_string),
        depends: values(op, "--requires").iter().map(|d| (*d).to_string()).collect(),
        spec: op.flag_value("--spec").map(str::to_string),
        review_url: op.flag_value("--review-url").map(str::to_string),
        body: prose(ctx, pending)?,
    };
    cmd_new(ctx, &opts)
}

/// `edit`, likewise — and here the body is the *whole* operation, so losing it would replay as
/// a commit that changes nothing.
fn replay_edit(ctx: &Ctx, iid: &str, pending: &str) -> Result<String, String> {
    let rows = load_rows(ctx)?;
    let iid = resolve_ref(&rows, iid)?;
    let row = rows.iter().find(|r| r.id == iid).ok_or_else(|| format!("no issue matching '{iid}'"))?.clone();
    let body = prose(ctx, pending)?;
    commit(ctx, rows, vec![Edit::Write { path: body_rel_path(&row), contents: body }], &Op::new("edit").operand(&iid))?;
    Ok(format!("#{iid} edited"))
}

/// The body a pending commit wrote.
///
/// Found by asking the commit which path it changed rather than deriving one from the op: an
/// `edit` op names no slug, and a `new` one names the slug it had *then*, which a rename landing
/// in between would have moved. The commit is the only thing that always knows.
fn prose(ctx: &Ctx, pending: &str) -> Result<String, String> {
    let Source::Ref { cwd, .. } = &ctx.source else {
        return Err("replay is only reachable for a ref-backed tracker".to_string());
    };
    let prefix = format!("{ITEMS_DIR}/");
    let changed = crate::git::changed_paths(cwd, pending)?;
    let mut bodies = changed.iter().filter(|p| p.starts_with(&prefix));
    let path = bodies.next().ok_or_else(|| format!("commit {pending} wrote no issue body to replay"))?;
    if bodies.next().is_some() {
        return Err(format!("commit {pending} wrote more than one issue body; it was not made by one verb"));
    }
    crate::git::show(cwd, pending, path)?.ok_or_else(|| format!("commit {pending} does not hold {path}"))
}

#[cfg(test)]
mod tests {
    // Tests assert; that is their job. The crate denies unwrap/expect/panic because a
    // malformed tracker must produce a diagnostic rather than a stack trace, but a test
    // that cannot panic cannot fail.
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    /// An op the engine cannot act on is a diagnostic naming it, not a panic — this is a
    /// commit message written by some other version of the engine.
    #[test]
    fn an_unknown_operation_is_refused_by_name() {
        let tmp = crate::discovery::tests::Tmp::new("replay-unknown");
        let ctx = Ctx::load(Source::Dir(tmp.tracker("issues")), false).expect("loads");
        let err = replay(&ctx, &Op::new("teleport").operand("aaaaaaa"), "deadbeef").expect_err("refused");
        assert!(err.contains("teleport"), "{err}");
    }

    /// An op that should name an issue and does not is refused before anything runs.
    #[test]
    fn an_operation_missing_its_issue_is_refused() {
        let tmp = crate::discovery::tests::Tmp::new("replay-noid");
        let ctx = Ctx::load(Source::Dir(tmp.tracker("issues")), false).expect("loads");
        let err = replay(&ctx, &Op::new("mv"), "deadbeef").expect_err("refused");
        assert!(err.contains("names no issue"), "{err}");
    }
}

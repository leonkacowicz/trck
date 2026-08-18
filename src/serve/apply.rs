//! Running a batch of staged edits, and saying what happened.
//!
//! **One writer at a time.** Every other verb is a process that resolves a tracker, writes
//! once, and exits; `serve` is the first thing in this engine that can have two writes in
//! flight at once, and two of them would race on more than the ref. The push path sets a
//! process-global flag while a rejected write is being rebuilt, so that a nested rebuild does
//! not start its own push loop — that flag is correct for one writer and meaningless for two.
//! A mutex held across the whole batch is the cheap and honest answer: writes here are typed
//! by a person, so there is nothing to gain by overlapping them and a whole class of
//! interleaving to lose.
//!
//! **A batch stops at the first refusal.** What it has already applied stays applied, because
//! each op is its own commit — which is exactly what pasting the panel's commands into a
//! terminal would do, and the response says which of them landed. The alternative would be
//! rewriting history to take one back, and a tracker branch other people have pulled is not
//! somewhere to do that.

use super::edits::{Edit, ops_for, parse_request};
use crate::discovery::Ctx;
use crate::json::Json;
use crate::verbs::Op;
use std::sync::{Mutex, PoisonError};

/// Serialises writes. See the note above: the push path's rebuild flag is process-global, so
/// two concurrent batches would not merely interleave commits, they would drop a push.
static WRITING: Mutex<()> = Mutex::new(());

/// What a batch did.
pub(crate) struct Outcome {
    /// What each applied operation reported, in order — the verb's own words.
    applied: Vec<String>,
    /// The refusal that stopped the batch, if one did.
    refused: Option<String>,
    /// The tracker ref after the batch. The page compares it with what it was rendered from,
    /// which is how it tells its own write from somebody else's.
    sha: Option<String>,
    /// Commits the remote has not got. A push that could not land is not a failed write — the
    /// commit is anchored locally — but it is not a shared one either, and a page that said
    /// nothing about it would be the offline story going silent.
    pending: usize,
}

impl Outcome {
    /// Whether every edit in the batch landed.
    pub(crate) fn ok(&self) -> bool {
        self.refused.is_none()
    }

    /// The answer as the page reads it.
    pub(crate) fn json(&self) -> String {
        let strings = |v: &[String]| Json::Array(v.iter().map(|s| Json::String(s.clone())).collect());
        Json::Object(vec![
            ("ok".into(), Json::Bool(self.ok())),
            ("applied".into(), strings(&self.applied)),
            ("error".into(), self.refused.clone().map_or(Json::Null, Json::String)),
            ("sha".into(), self.sha.clone().map_or(Json::Null, Json::String)),
            ("pending".into(), Json::Number(self.pending.to_string())),
        ])
        .to_json()
    }
}

/// Apply the batch in `body`, or say why it is not a batch.
///
/// The `Err` is for a request that is not a request — malformed JSON, a missing key. A batch
/// that *is* well formed and gets refused by the tracker comes back as an `Outcome` carrying
/// the engine's own diagnostic, because that is an answer about the tracker rather than about
/// the request.
pub(crate) fn batch(ctx: &Ctx, body: &str) -> Result<Outcome, String> {
    let edits = parse_request(body)?;
    // Held for the whole batch, not per op: the ops within one batch are meant to land in the
    // order the page staged them, and another writer slipping between two of them would make
    // the second one derive from a tracker the first never saw.
    let _writing = WRITING.lock().unwrap_or_else(PoisonError::into_inner);
    let mut outcome = Outcome { applied: Vec::new(), refused: None, sha: None, pending: 0 };
    for edit in &edits {
        if let Err(refusal) = apply_one(ctx, edit, &mut outcome.applied) {
            outcome.refused = Some(refusal);
            break;
        }
    }
    outcome.sha = head_sha(ctx);
    outcome.pending = unpushed(ctx);
    Ok(outcome)
}

/// One edit: resolve the issue it names, then run every op it means.
///
/// The row is looked up per edit rather than once for the batch, because each op commits and
/// the next edit has to see what the last one did — the ops a list field means are a
/// difference, and a difference against a stale row is the wrong difference.
fn apply_one(ctx: &Ctx, edit: &Edit, applied: &mut Vec<String>) -> Result<(), String> {
    let rows = crate::verbs::load_rows(ctx)?;
    let id = crate::verbs::resolve_ref(&rows, &edit.id)?;
    let row = rows.iter().find(|r| r.id == id).ok_or_else(|| format!("no issue matching '{id}'"))?;
    for op in ops_for(row, &edit.field, &edit.value)? {
        applied.push(run(ctx, &op)?);
    }
    Ok(())
}

/// Run one op through the verb the CLI would have called.
///
/// [`crate::verbs::replay`] already dispatches an op to its verb — that is how a rejected push
/// re-runs what it could not land — but it takes the commit the op's prose came from, which
/// only `new` and `edit` have and neither is reachable from here. So this is the same dispatch
/// with prose left out, and the two verbs that need it refused by name rather than dispatched
/// into a missing commit.
fn run(ctx: &Ctx, op: &Op) -> Result<String, String> {
    let id = op.operands.first().map(String::as_str).ok_or_else(|| format!("`{}` names no issue", op.render()))?;
    match op.verb.as_str() {
        "mv" => {
            let status = op.operands.get(1).ok_or_else(|| format!("`{}` names no status", op.render()))?;
            let opts = crate::verbs::MvOpts { status, resolution: op.flag_value("--resolution"), review_url: op.flag_value("--review-url") };
            crate::verbs::cmd_mv(ctx, id, &opts)
        },
        "set" => crate::verbs::cmd_set(ctx, id, &set_opts(op)),
        "label" => crate::verbs::cmd_label(ctx, id, &flag_values(op, "--add"), &flag_values(op, "--remove")),
        "dep" => crate::verbs::cmd_dep(ctx, id, op.flag_value("--add"), op.flag_value("--remove")),
        // Unreachable from `ops_for`, whose field list produces only the four above. Stated
        // rather than left to a catch-all, because the day a field is added the compiler will
        // not be the one to notice and this message will.
        other => Err(format!("`{other}` is not an operation this page can ask for")),
    }
}

/// `set`'s options, from the one flag an edit ever produces.
fn set_opts(op: &Op) -> crate::verbs::SetOpts<'_> {
    crate::verbs::SetOpts {
        priority: op.flag_value("--priority"),
        points: op.flag_value("--points").and_then(|p| p.parse().ok()),
        parent: op.flag_value("--parent"),
        ..crate::verbs::SetOpts::default()
    }
}

fn flag_values<'a>(op: &'a Op, name: &str) -> Vec<&'a str> {
    op.flags.iter().filter(|(n, _)| n == name).filter_map(|(_, v)| v.as_deref()).collect()
}

/// Commits the remote has not got.
///
/// A count that cannot be taken is not worth failing a completed write over — the same
/// judgement the CLI's pending note makes, for the same reason.
fn unpushed(ctx: &Ctx) -> usize {
    let crate::discovery::Source::Ref { cwd, .. } = &ctx.source else {
        return 0;
    };
    crate::discovery::standing::pending(cwd).unwrap_or(0)
}

/// What the tracker ref points at now.
///
/// `None` for a directory tracker, which has no ref and no sha to compare — the page falls
/// back to reloading, which is what it would do anyway.
fn head_sha(ctx: &Ctx) -> Option<String> {
    let crate::discovery::Source::Ref { cwd, .. } = &ctx.source else {
        return None;
    };
    crate::git::rev_parse(cwd, crate::verbs::backend::local_branch(crate::discovery::TRACKER_REF)).ok().flatten()
}

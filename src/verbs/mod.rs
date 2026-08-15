//! The mutating verbs, and the write path every one of them ends in.
//!
//! Three things are shared by all of them and worth reading first.
//!
//! [`finalize`] is the single derivation: work out what is derived, render the index, render
//! the summary. Deriving on write rather than in each verb is what makes the rollup uniform
//! across `mv`, `start`, `done`, `new --parent` and re-parenting with no per-command hooks.
//!
//! It **derives without writing**. What comes back is a [`Changeset`] — the two generated
//! files plus whatever the verb does to a body — and an [`Op`], the verb's own account of what
//! it was asked to do. [`commit`] hands both to a backend. A tracker that lives in a git ref
//! (`#sqzr7nk`) is a second `apply`, not a second copy of the verbs.
//!
//! Writes are **atomic**: a temporary file in the same directory, then a rename. An
//! interrupted run leaves the previous index intact rather than half a line, which
//! matters because the index is the tracker's only source of truth.

mod backend;
mod changeset;
mod clock;
mod edit;
mod finalize;
mod op;
mod replay;
mod slug;
mod status;
mod write;

pub(crate) use backend::{DirBackend, RefBackend};
pub(crate) use changeset::{Changeset, Edit};
pub(crate) use clock::now_utc;
pub(crate) use edit::{MvOpts, NewOpts, SetOpts, cmd_dep, cmd_label, cmd_mv, cmd_new, cmd_set};
pub(crate) use finalize::finalize;
pub(crate) use op::Op;
pub(crate) use replay::replay;
pub(crate) use slug::{check_slug, slugify};
pub(crate) use status::apply_status;
pub(crate) use write::{write_atomic, write_file};

use crate::discovery::content::SUMMARY_NAME;
use crate::discovery::{Ctx, ITEMS_DIR, Source};
use crate::index::parse_index;
use crate::issue::Issue;
use crate::summary::filename;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

/// Derive, apply, and say so if the result is inconsistent — the tail every mutating verb
/// shares.
///
/// Splitting it from [`finalize`] is what keeps the derivation testable without a temporary
/// directory: everything above this line is values, and everything a filesystem sees goes
/// through the one `apply` below it.
pub(crate) fn commit(ctx: &Ctx, rows: Vec<Issue>, body: Vec<Edit>, op: &Op) -> Result<Vec<Issue>, String> {
    let cs = finalize(rows, body)?;
    apply(ctx, &cs, op)?;
    report_inconsistencies(ctx, &cs.rows);
    share(ctx, op)?;
    Ok(cs.rows)
}

/// Set while a rejected write is being rebuilt.
///
/// Rebuilding re-runs the verb, which lands back in [`commit`] — and a nested push loop would
/// have every rejection start another one, each retrying the last. The outer loop is the only
/// one that should push: the inner run's job is to *build* the commit on the new base, and the
/// loop that asked for it pushes what it produced.
static REBUILDING: AtomicBool = AtomicBool::new(false);

/// Get the commit onto the remote, rebuilding onto whatever landed first if it moved.
///
/// A directory tracker has no remote of its own — it is files in someone's checkout, shared by
/// whatever commits that checkout — so this is the ref backend's alone.
fn share(ctx: &Ctx, op: &Op) -> Result<(), String> {
    let Source::Ref { rev, cwd } = &ctx.source else {
        return Ok(());
    };
    if REBUILDING.load(Ordering::Relaxed) {
        return Ok(());
    }
    REBUILDING.store(true, Ordering::Relaxed);
    let result = backend::sync(cwd, &backend::local_ref(rev), op, &|op, pending| replay(ctx, op, pending));
    REBUILDING.store(false, Ordering::Relaxed);
    result
}

/// Hand the changeset to whichever backend this tracker is.
///
/// The one place the two kinds of tracker are told apart. Every verb above it works on values
/// and never learns which it was operating on, which is what kept adding the second one from
/// being a rewrite of the verbs.
fn apply(ctx: &Ctx, cs: &Changeset, op: &Op) -> Result<(), String> {
    match &ctx.source {
        Source::Dir(dir) => DirBackend::new(dir).apply(cs, op),
        Source::Ref { rev, cwd } => RefBackend::new(cwd, rev).apply(cs, op),
    }
}

/// Regenerate `SUMMARY.md` alone.
///
/// `summary` is the one mutating verb that leaves the index untouched — it re-renders what the
/// index already says. So it gets its own one-edit changeset rather than [`commit`]'s, which
/// would also rewrite and re-derive rows the user did not ask to change. It still goes through
/// a changeset, because "no verb writes a file outside one" is what makes a second backend a
/// complete backend.
pub(crate) fn write_summary(ctx: &Ctx, g: &crate::graph::Graph) -> Result<(), String> {
    let contents = crate::summary::generate_summary(g);
    let cs = Changeset::new(Vec::new(), vec![Edit::Write { path: PathBuf::from(SUMMARY_NAME), contents }]);
    apply(ctx, &cs, &Op::new("summary"))
}

/// Validate what was just written, reusing the rows rather than re-parsing.
///
/// A verb that leaves the tracker inconsistent still succeeds — it did what it was asked — but
/// says so loudly, because the next thing that runs is usually a commit.
fn report_inconsistencies(ctx: &Ctx, rows: &[Issue]) {
    let Ok(report) = crate::validate::validate(ctx, rows) else {
        return;
    };
    for w in &report.warnings {
        eprintln!("warning: {w}");
    }
    if report.errors.is_empty() {
        return;
    }
    eprintln!("\nINCONSISTENCIES after this operation:");
    for e in &report.errors {
        eprintln!("  error: {e}");
    }
    eprintln!("the tracker is now inconsistent — fix before committing.");
}

/// Where a row's body lives, as a changeset addresses it: relative to the tracker.
///
/// [`issue_path`] answers the same question absolutely, and both exist because a verb needs
/// the absolute one to *report* a path to the user and the relative one to *change* it.
pub(crate) fn body_rel_path(row: &Issue) -> PathBuf {
    PathBuf::from(ITEMS_DIR).join(filename(row))
}

/// The prose skeleton a new issue's body starts from.
pub(super) const TEMPLATE: &str = r"# {title}

## Summary
<!-- What needs doing and why. For an epic, link the spec instead of re-narrating it. -->

## Acceptance criteria
- [ ]

## Notes
<!-- Context, links to files/commits, open questions, decisions. -->
";

/// Where an issue's body lives on disk.
///
/// Fallible because a ref-backed tracker has no such place, and the alternative — a
/// relative path that resolves against the caller's working directory — is a plausible
/// answer that is wrong.
pub(crate) fn issue_path(ctx: &Ctx, row: &Issue) -> Result<PathBuf, String> {
    Ok(ctx.items_dir()?.join(filename(row)))
}

/// How a verb names an issue's body back to whoever ran it.
///
/// A directory tracker answers with the file to open. A ref-backed one has no such file, so
/// it answers the way git spells the same thing — `<rev>:items/<id>-<slug>.md`. Deliberately
/// not a bare relative path: that reads as real, resolves against whatever directory the
/// caller happens to be in, and is not there — the trap already refused for `trck path`.
pub(crate) fn body_location(ctx: &Ctx, row: &Issue) -> String {
    let name = filename(row);
    match &ctx.source {
        Source::Dir(dir) => dir.join(ITEMS_DIR).join(name).display().to_string(),
        Source::Ref { rev, .. } => format!("{rev}:{ITEMS_DIR}/{name}"),
    }
}

/// Does the tracker already hold a body under this name?
///
/// Asked of the tracker rather than of a filesystem, so `new`'s collision guard holds for a
/// ref-backed tracker too — where the "file" is a tree entry and `exists()` would be a
/// question about the caller's working directory instead.
pub(crate) fn body_taken(ctx: &Ctx, row: &Issue) -> bool {
    ctx.list_items().is_ok_and(|names| names.contains(&filename(row)))
}

pub(crate) fn load_rows(ctx: &Ctx) -> Result<Vec<Issue>, String> {
    parse_index(&ctx.read_index()?, "index.jsonl")
}

/// Resolve a CLI id token to exactly one issue: exact id, then unique prefix.
pub(crate) fn resolve_ref(rows: &[Issue], token: &str) -> Result<String, String> {
    let token = token.strip_prefix('#').unwrap_or(token);
    if rows.iter().any(|r| r.id == token) {
        return Ok(token.to_string());
    }
    let hits: Vec<&str> = rows.iter().map(|r| r.id.as_str()).filter(|id| id.starts_with(token)).collect();
    match hits.len() {
        1 => Ok(hits[0].to_string()),
        0 => Err(format!("no issue matching '{token}'")),
        _ => {
            let mut cands = hits;
            cands.sort_unstable();
            Err(format!("ambiguous id prefix '{token}' matches: {}", cands.join(", ")))
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
    use crate::test_graph::issue;

    #[test]
    fn resolve_ref_takes_an_exact_id_then_a_unique_prefix() {
        let rows = vec![issue("aaaaaaa"), issue("aabbbbb")];
        assert_eq!(resolve_ref(&rows, "aaaaaaa").expect("exact"), "aaaaaaa");
        assert_eq!(resolve_ref(&rows, "#aaaaaaa").expect("hash"), "aaaaaaa");
        assert_eq!(resolve_ref(&rows, "aab").expect("prefix"), "aabbbbb");
        assert!(resolve_ref(&rows, "aa").expect_err("ambiguous").contains("ambiguous"));
        assert!(resolve_ref(&rows, "zz").expect_err("none").contains("no issue"));
    }

    /// An exact id wins even when it is also a prefix of another — otherwise the shorter id
    /// in a pair like `ab`/`abcd` would be unreachable.
    #[test]
    fn an_exact_id_beats_being_a_prefix_of_another() {
        let rows = vec![issue("ab"), issue("abcd")];
        assert_eq!(resolve_ref(&rows, "ab").expect("exact wins"), "ab");
    }

    /// An empty tracker resolves nothing rather than matching everything: the prefix filter
    /// would otherwise make `""` ambiguous or, with one row, a silent hit.
    #[test]
    fn nothing_resolves_against_no_rows() {
        assert!(resolve_ref(&[], "aaaaaaa").is_err());
    }
}

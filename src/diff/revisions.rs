//! Turning a revision spec, a file or standard input into a snapshot to diff against.
//!
//! Split from [`super`], which is about what a difference *is*. This is about where the two
//! sides come from — and it is the half the tracker's move to its own branch changes, since a
//! revision of the checkout stops being a revision of the tracker.

use super::Snapshot;
use crate::discovery::{Ctx, Source};
use std::path::Path;

const USE_FROM: &str = "use --from/--to with file paths instead";

/// Why a git failure is not fatal to `diff` itself: the file-based sources still work, and
/// saying so is more use than reporting that a spawn failed.
///
/// The [`crate::git`] primitives are deliberately unphrased, so the sentence is added here —
/// only this caller knows that `--from`/`--to` is the way around it.
fn unavailable(_: String) -> String {
    format!("git is not on PATH, so revision specs are unavailable; {USE_FROM}")
}

/// The tracker as of `rev`.
///
/// A tracker dir absent at that revision is **not** an error for a directory-backed tracker:
/// comparing against a commit from before the tracker existed is a legitimate question, and
/// the answer is "every issue is new". It *is* an error for a ref-backed one, where every
/// commit on the branch holds an index by construction — so a revision with none is a
/// revision of something else, and an empty diff would be a confident wrong answer.
///
/// An unresolvable revision is an error either way, reported separately, so "you typo'd the
/// branch" stays distinguishable from "the tracker did not exist yet".
pub(crate) fn git_snapshot(ctx: &Ctx, rev: &str) -> Result<Snapshot, String> {
    let asked = anchored(ctx, rev);
    if crate::git::rev_parse(ctx.git_cwd(), &asked).map_err(unavailable)?.is_none() {
        return Err(format!("unknown revision '{rev}'"));
    }
    let prefix = ctx.tracker_prefix().map_err(unavailable)?.ok_or_else(|| format!("not a git repository, so revision specs are unavailable; {USE_FROM}"))?;
    let held = crate::git::show(ctx.git_cwd(), &asked, &format!("{prefix}index.jsonl")).map_err(unavailable)?;
    match (held, &ctx.source) {
        (Some(text), _) => Snapshot::from_text(&text, rev),
        (None, Source::Dir(_)) => Snapshot::from_text("", rev),
        (None, Source::Ref { .. }) => Err(format!(
            "revision '{rev}' holds no index.jsonl, so it is not a revision of this tracker \
             — the tracker's history is on `{asked}`'s branch, not this checkout's"
        )),
    }
}

/// A revision spec as it means something to *this* tracker.
///
/// For a directory-backed tracker the checkout's revisions are the tracker's: it is a subtree
/// of that history, and `HEAD` is where the working tree is. For a ref-backed one they are
/// not — `HEAD~5` on `main` is five commits of engine code and says nothing about any issue —
/// so `HEAD` means the tracker branch's tip and anything anchored on it counts tracker
/// commits.
///
/// Only `HEAD` is rewritten. A sha, a tag or a branch name means what it says in either kind
/// of tracker, and rewriting those would make `trck diff <sha>` unable to name a commit.
fn anchored(ctx: &Ctx, rev: &str) -> String {
    match &ctx.source {
        Source::Dir(_) => rev.to_string(),
        Source::Ref { rev: tracker, .. } => reanchor(rev, tracker),
    }
}

/// `HEAD`, and only `HEAD`, replaced by `tracker`.
fn reanchor(rev: &str, tracker: &str) -> String {
    let Some(suffix) = rev.strip_prefix("HEAD") else {
        return rev.to_string();
    };
    // `HEAD`, `HEAD~2`, `HEAD^`, `HEAD@{1}` — but not a branch called `HEADROOM`.
    if suffix.is_empty() || suffix.starts_with(['~', '^', '@']) { format!("{tracker}{suffix}") } else { rev.to_string() }
}

/// Split a revision spec into `(old, new)`; a `None` new side means the working tree.
pub(crate) fn parse_rev_spec(spec: &str) -> Result<(String, Option<String>), String> {
    if spec.contains("...") {
        return Err("three-dot (merge-base) revision specs are not supported; \
                    use `a..b` to compare two revisions directly"
            .to_string());
    }
    let Some((old, new)) = spec.split_once("..") else {
        return Ok((spec.to_string(), None));
    };
    if old.is_empty() || new.is_empty() {
        return Err(format!("incomplete revision range '{spec}'; both sides of `..` are required"));
    }
    Ok((old.to_string(), Some(new.to_string())))
}

/// Resolve a `--from`/`--to` spec: a file, a directory holding one, `-` for stdin, or the
/// working tree when unspecified.
pub(crate) fn resolve_source(spec: Option<&str>, ctx: &Ctx) -> Result<Snapshot, String> {
    let Some(spec) = spec else {
        return Snapshot::from_text(&ctx.read_index()?, "working tree");
    };
    if spec == "-" {
        use std::io::Read as _;
        let mut text = String::new();
        std::io::stdin().read_to_string(&mut text).map_err(|e| format!("stdin: {e}"))?;
        return Snapshot::from_text(&text, "stdin");
    }
    let path = Path::new(spec);
    // The label is the file's own name, not the spec that named it: a long relative path
    // buries the one word that identifies the side being compared.
    let label = path.file_name().map_or_else(|| spec.to_string(), |n| n.to_string_lossy().into_owned());
    if path.is_dir() {
        // A tracker dir with no index is an empty snapshot, not an error: the tracker
        // not existing on one side is a legitimate comparison, and everything on the
        // other side reads as added.
        let text = std::fs::read_to_string(path.join("index.jsonl")).unwrap_or_default();
        return Snapshot::from_text(&text, &label);
    }
    let text = std::fs::read_to_string(path).map_err(|_| format!("no such file: {spec}"))?;
    Snapshot::from_text(&text, &label)
}

// --------------------------------------------------------------------------- //
// changelog
// --------------------------------------------------------------------------- //

#[cfg(test)]
mod tests {
    // Tests assert; that is their job. The crate denies unwrap/expect/panic because a
    // malformed tracker must produce a diagnostic rather than a stack trace, but a test
    // that cannot panic cannot fail.
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::{parse_rev_spec, reanchor};

    #[test]
    fn head_and_everything_anchored_on_it_moves_to_the_tracker() {
        assert_eq!(reanchor("HEAD", "trck-issues"), "trck-issues");
        assert_eq!(reanchor("HEAD~5", "trck-issues"), "trck-issues~5");
        assert_eq!(reanchor("HEAD^", "trck-issues"), "trck-issues^");
        assert_eq!(reanchor("HEAD@{1}", "trck-issues"), "trck-issues@{1}");
    }

    /// A sha, a tag or a branch means what it says. Rewriting those would leave `trck diff
    /// <sha>` unable to name a commit.
    #[test]
    fn anything_else_is_left_alone() {
        assert_eq!(reanchor("main", "trck-issues"), "main");
        assert_eq!(reanchor("v0.29.1", "trck-issues"), "v0.29.1");
        assert_eq!(reanchor("deadbeef", "trck-issues"), "deadbeef");
    }

    /// The prefix match has to be a word, not a string: a branch may be named after it.
    #[test]
    fn a_branch_whose_name_starts_with_head_is_not_head() {
        assert_eq!(reanchor("HEADROOM", "trck-issues"), "HEADROOM");
        assert_eq!(reanchor("HEAD-of-line", "trck-issues"), "HEAD-of-line");
    }

    #[test]
    fn a_revision_range_names_both_sides() {
        assert_eq!(parse_rev_spec("HEAD").expect("ok"), ("HEAD".into(), None));
        assert_eq!(parse_rev_spec("a..b").expect("ok"), ("a".to_string(), Some("b".to_string())));
        assert!(parse_rev_spec("a...b").is_err(), "merge-base specs are refused");
        assert!(parse_rev_spec("a..").is_err());
        assert!(parse_rev_spec("..b").is_err());
    }
}

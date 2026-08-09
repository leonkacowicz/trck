//! The mutating verbs, and the write path every one of them ends in.
//!
//! Two things are shared by all of them and worth reading first.
//!
//! [`finalize`] is the single write path: derive what is derived, write the index, write
//! the summary. Deriving on write rather than in each verb is what makes the rollup
//! uniform across `mv`, `start`, `done`, `new --parent` and re-parenting with no
//! per-command hooks.
//!
//! Writes are **atomic**: a temporary file in the same directory, then a rename. An
//! interrupted run leaves the previous index intact rather than half a line, which
//! matters because the index is the tracker's only source of truth.

mod clock;
mod edit;
mod finalize;
mod slug;
mod status;
mod write;

pub(crate) use clock::now_utc;
pub(crate) use edit::{MvOpts, NewOpts, SetOpts, cmd_dep, cmd_label, cmd_mv, cmd_new, cmd_set};
pub(crate) use finalize::finalize;
pub(crate) use slug::{check_slug, slugify};
pub(crate) use status::apply_status;
pub(crate) use write::{write_atomic, write_file};

use crate::discovery::Ctx;
use crate::index::parse_index;
use crate::issue::Issue;
use crate::summary::filename;
use std::path::PathBuf;

/// The prose skeleton a new issue's body starts from.
pub(super) const TEMPLATE: &str = "# {title}\n\
    \n\
    ## Summary\n\
    <!-- What needs doing and why. For an epic, link the spec instead of re-narrating it. -->\n\
    \n\
    ## Acceptance criteria\n\
    - [ ]\n\
    \n\
    ## Notes\n\
    <!-- Context, links to files/commits, open questions, decisions. -->\n";

pub(crate) fn issue_path(ctx: &Ctx, row: &Issue) -> PathBuf {
    ctx.items_dir().join(filename(row))
}

pub(crate) fn load_rows(ctx: &Ctx) -> Result<Vec<Issue>, String> {
    let path = ctx.index_path();
    match std::fs::read_to_string(&path) {
        Ok(text) => parse_index(&text, "index.jsonl"),
        Err(_) => Ok(Vec::new()),
    }
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

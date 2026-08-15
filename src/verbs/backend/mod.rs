//! Where a [`Changeset`] lands.
//!
//! Everything upstream of here — deriving the rollup, rendering the index, working out which
//! body file moved — happens on values. A backend is the only thing that makes any of it
//! real, which is what lets a tracker be a directory or a git ref without the verbs knowing
//! which.
//!
//! [`dir`] writes files. [`git`] builds a commit and moves a ref, touching neither the
//! working tree nor the caller's index — so a tracker write works from a dirty checkout on an
//! unrelated branch, which is the whole reason the tracker moves off one.
//!
//! Both take the [`Op`] alongside the changeset. A directory has nowhere to record what the
//! verb was asked to do and drops it; a commit carries it in the message, which is what makes
//! the history replayable rather than merely diffable.

use super::changeset::{Changeset, Edit};
use super::op::Op;

mod dir;
mod git;
// Writing a message is [`git`]'s; reading one back is nobody's yet. The replay path
// (`#5w9d7sq`) is `op_of`'s first consumer, and re-exporting it from here is that change's to
// make — which is what the crate-level `dead_code` expectation covers in the meantime.
mod message;

pub(crate) use dir::DirBackend;
pub(crate) use git::RefBackend;

/// A changeset path as git spells it.
///
/// Changeset paths are built with `PathBuf::join`, so on Windows they arrive with backslashes
/// — and a tree entry named `items\a-a.md` is one path component containing a backslash, not
/// a file in `items/`. The tracker would still round-trip through itself and be unreadable to
/// everything else, which is the kind of bug that only shows up on the platform nobody
/// developing it runs.
fn git_path(path: &std::path::Path) -> String {
    path.components().filter_map(|c| c.as_os_str().to_str()).collect::<Vec<&str>>().join("/")
}

#[cfg(test)]
mod tests {
    // Tests assert; that is their job. The crate denies unwrap/expect/panic because a
    // malformed tracker must produce a diagnostic rather than a stack trace, but a test
    // that cannot panic cannot fail.
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;
    use std::path::PathBuf;

    /// Forward slashes whatever the platform's separator is — a tree entry is a git path,
    /// not a local one.
    #[test]
    fn a_changeset_path_becomes_a_git_path() {
        assert_eq!(git_path(&PathBuf::from("index.jsonl")), "index.jsonl");
        assert_eq!(git_path(&PathBuf::from("items").join("aaaaaaa-a.md")), "items/aaaaaaa-a.md");
    }
}

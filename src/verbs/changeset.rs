//! What a mutating verb produces instead of writing files.
//!
//! A verb derives a whole new tracker state and then has to put it somewhere. Splitting the
//! two — [`Changeset`] for the bytes, [`super::op::Op`] for the intent — is what lets a second
//! destination exist: a directory applies the changeset with `write`/`rename`/`remove`, and a
//! commit-building backend (`#sqzr7nk`) turns the same edits into blobs and a tree, with the
//! `Op` as the commit's replayable record of what was asked for.
//!
//! Paths here are **tracker-relative** (`index.jsonl`, `items/aaaaaaa-a-title.md`). An
//! absolute path is a fact about one backend; a tracker that lives in a git ref has a tree to
//! address, not a directory.

use crate::issue::Issue;
use std::path::PathBuf;

/// One file's worth of change.
///
/// `Rename` is its own variant rather than a delete plus a write because the two are not the
/// same thing to the destination: git records a rename as a rename, and `set --slug` moving a
/// body must not read as an unrelated file appearing and another vanishing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Edit {
    Write { path: PathBuf, contents: String },
    Rename { from: PathBuf, to: PathBuf },
    Delete { path: PathBuf },
}

/// Everything a verb changed, in the order it must be applied.
///
/// `rows` is the derived index the edits already encode — carried alongside because the
/// caller validates against rows, not against rendered text it would have to re-parse.
#[derive(Debug, Default)]
pub(crate) struct Changeset {
    pub(crate) rows: Vec<Issue>,
    pub(crate) edits: Vec<Edit>,
}

impl Changeset {
    pub(crate) fn new(rows: Vec<Issue>, edits: Vec<Edit>) -> Self {
        Self { rows, edits }
    }
}

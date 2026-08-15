//! A tracker that is a git ref: the changeset applied as one commit.
//!
//! Blobs, then a tree, then `commit-tree`, then a compare-and-swap onto the local branch.
//! Nothing is checked out and nothing is staged — `GIT_INDEX_FILE` points the tree build at a
//! scratch index, which is the whole reason this is not `git add`. A tracker write therefore
//! works from a dirty working tree on an unrelated branch, and leaves both exactly as it
//! found them.
//!
//! **The local branch is the write-ahead log, not a convenience.** The commit is anchored by a
//! ref before anything is pushed, so a failed push leaves the issue that was just filed in the
//! object store rather than unreferenced and one `gc` from gone. Pushing it is `#5w9d7sq`.
//!
//! **A tree is built from an empty index**, so it holds exactly what it is given. The
//! changeset names the handful of files a verb touched; everything else the base commit held
//! has to be carried forward explicitly, which is what [`plan`] does.

use super::super::changeset::Op;
use super::{Changeset, Edit, git_path};
use crate::git::write::{commit_tree, hash_object, update_ref, write_tree};
use crate::git::{rev_parse, tree_blobs};
use std::collections::BTreeMap;
use std::path::Path;

/// A tracker read out of, and written back to, a git ref.
pub(crate) struct RefBackend<'a> {
    /// Any directory inside the repository holding the ref.
    cwd: &'a Path,
    /// The revision the tracker was opened as — `trck-issues`, `origin/trck-issues`, or
    /// whatever `--ref` named.
    rev: &'a str,
}

impl<'a> RefBackend<'a> {
    pub(crate) fn new(cwd: &'a Path, rev: &'a str) -> Self {
        Self { cwd, rev }
    }

    /// Turn the changeset into one commit and move the local branch onto it.
    pub(crate) fn apply(&self, cs: &Changeset, op: &Op) -> Result<(), String> {
        let target = local_ref(self.rev);
        // Two separate questions. The *parent* is whatever the tracker currently reads as,
        // which on a fresh clone is the remote-tracking ref. The *expectation* is what the
        // local branch holds, which on that same clone is nothing — so the first write there
        // creates the branch at a commit whose parent is the remote's tip.
        let held = rev_parse(self.cwd, &target)?;
        let parent = match &held {
            Some(sha) => Some(sha.clone()),
            None => rev_parse(self.cwd, self.rev)?,
        };
        let entries = self.plan_tree(parent.as_deref(), cs)?;
        let listed: Vec<(&str, &str)> = entries.iter().map(|(p, sha)| (p.as_str(), sha.as_str())).collect();
        let tree = write_tree(self.cwd, &listed)?;
        let parents: Vec<&str> = parent.as_deref().into_iter().collect();
        let commit = commit_tree(self.cwd, &tree, &parents, &message(op)).map_err(explain_identity)?;
        update_ref(self.cwd, &target, &commit, held.as_deref())
    }

    /// Every path the new tree holds, with the blob at each.
    ///
    /// The writes are hashed first so that [`plan`] — where a rename moves an entry and a
    /// delete removes one — stays a function of values and can be tested without a
    /// repository.
    fn plan_tree(&self, parent: Option<&str>, cs: &Changeset) -> Result<BTreeMap<String, String>, String> {
        let base = match parent {
            Some(sha) => tree_blobs(self.cwd, sha)?.into_iter().collect(),
            None => BTreeMap::new(),
        };
        let mut blobs = Vec::with_capacity(cs.edits.len());
        for edit in &cs.edits {
            blobs.push(match edit {
                Edit::Write { contents, .. } => Some(hash_object(self.cwd, contents)?),
                _ => None,
            });
        }
        Ok(plan(base, &cs.edits, &blobs))
    }
}

/// The base's entries with every edit applied, in order.
///
/// A rename carries the blob across rather than re-hashing it: the bytes did not change, and
/// moving the entry is what makes git record the change as a rename. A rename of a path the
/// base does not hold drops out — the directory backend would have failed there, but a
/// changeset is not a patch and refusing to build a tree over a body that a hand-edit already
/// moved would strand the tracker rather than repair it.
fn plan(mut base: BTreeMap<String, String>, edits: &[Edit], blobs: &[Option<String>]) -> BTreeMap<String, String> {
    for (edit, blob) in edits.iter().zip(blobs) {
        match edit {
            Edit::Write { path, .. } => {
                if let Some(sha) = blob {
                    base.insert(git_path(path), sha.clone());
                }
            },
            Edit::Rename { from, to } => {
                if let Some(sha) = base.remove(&git_path(from)) {
                    base.insert(git_path(to), sha);
                }
            },
            Edit::Delete { path } => {
                base.remove(&git_path(path));
            },
        }
    }
    base
}

/// The local branch a revision writes to.
///
/// Writes always land on `refs/heads/`, whatever the tracker was *read* from: a
/// remote-tracking ref is a copy of someone else's branch and moving it locally would make
/// this clone disagree with the remote it is named after. Stripping `origin/` is what turns a
/// fresh clone's only ref into the branch this write should create.
fn local_ref(rev: &str) -> String {
    let name = rev.strip_prefix("refs/heads/").or_else(|| rev.strip_prefix("origin/")).unwrap_or(rev);
    format!("refs/heads/{name}")
}

/// The commit message.
///
/// The op's own rendering, which is enough to say what happened and to replay it. The subject
/// convention and the `Trck-Op` trailer that makes replay machine-readable are `#93zhqbd`;
/// this is deliberately the smallest thing that produces a legible history in the meantime.
fn message(op: &Op) -> String {
    format!("{}\n", op.render())
}

/// Turn `commit-tree`'s identity refusal into one that names the remedy.
///
/// git's own version is four lines of shell aimed at someone who is committing by hand, and
/// it ends in `unable to auto-detect email address` — which reads as a bug in trck rather
/// than as a machine that has never had git configured.
fn explain_identity(err: String) -> String {
    if !err.contains("auto-detect") && !err.contains("tell me who you are") {
        return err;
    }
    "git has no commit identity, so the tracker commit cannot be made. Set one with:\n  \
     git config --global user.name \"Your Name\"\n  \
     git config --global user.email \"you@example.com\""
        .to_string()
}

#[cfg(test)]
mod tests {
    // Tests assert; that is their job. The crate denies unwrap/expect/panic because a
    // malformed tracker must produce a diagnostic rather than a stack trace, but a test
    // that cannot panic cannot fail.
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;
    use std::path::PathBuf;

    fn base() -> BTreeMap<String, String> {
        [("index.jsonl".to_string(), "i0".to_string()), ("items/a-old.md".to_string(), "b0".to_string())].into_iter().collect()
    }

    /// Everything the base held that the changeset never mentions is still there. This is
    /// what a tree built from an empty index would otherwise silently drop.
    #[test]
    fn untouched_files_are_carried_forward() {
        let edits = vec![Edit::Write { path: PathBuf::from("index.jsonl"), contents: String::new() }];
        let out = plan(base(), &edits, &[Some("i1".to_string())]);
        assert_eq!(out.get("items/a-old.md"), Some(&"b0".to_string()), "the body nobody touched survives");
        assert_eq!(out.get("index.jsonl"), Some(&"i1".to_string()), "and the index is the new one");
    }

    /// A rename moves the entry and carries its blob: the bytes did not change, and re-adding
    /// them under the new name is what would make git read it as an unrelated file.
    #[test]
    fn a_rename_moves_the_entry_and_keeps_its_blob() {
        let edits = vec![Edit::Rename { from: PathBuf::from("items/a-old.md"), to: PathBuf::from("items/a-new.md") }];
        let out = plan(base(), &edits, &[None]);
        assert_eq!(out.get("items/a-new.md"), Some(&"b0".to_string()));
        assert!(!out.contains_key("items/a-old.md"), "the old name is gone");
    }

    #[test]
    fn a_delete_removes_the_entry() {
        let edits = vec![Edit::Delete { path: PathBuf::from("items/a-old.md") }];
        let out = plan(base(), &edits, &[None]);
        assert!(!out.contains_key("items/a-old.md"));
        assert_eq!(out.len(), 1, "and nothing else moved");
    }

    /// `set --slug --title` renames then writes, and order decides which name survives.
    #[test]
    fn a_rename_followed_by_a_write_lands_on_the_new_name() {
        let edits = vec![
            Edit::Rename { from: PathBuf::from("items/a-old.md"), to: PathBuf::from("items/a-new.md") },
            Edit::Write { path: PathBuf::from("items/a-new.md"), contents: String::new() },
        ];
        let out = plan(base(), &edits, &[None, Some("b1".to_string())]);
        assert_eq!(out.get("items/a-new.md"), Some(&"b1".to_string()), "the rewrite wins");
        assert!(!out.contains_key("items/a-old.md"));
    }

    /// A body a hand-edit already moved leaves nothing to rename. Refusing here would strand
    /// the tracker; the tree is still built, and `check` is what reports the stray file.
    #[test]
    fn a_rename_of_a_path_the_base_lacks_is_dropped() {
        let edits = vec![Edit::Rename { from: PathBuf::from("items/gone.md"), to: PathBuf::from("items/a-new.md") }];
        let out = plan(base(), &edits, &[None]);
        assert!(!out.contains_key("items/a-new.md"), "nothing to move, so nothing appears");
        assert_eq!(out, base(), "and the tree is otherwise untouched");
    }

    /// The first write to a tracker with no ref builds its whole tree from the changeset.
    #[test]
    fn with_no_base_the_changeset_is_the_whole_tree() {
        let edits = vec![Edit::Write { path: PathBuf::from("index.jsonl"), contents: String::new() }];
        let out = plan(BTreeMap::new(), &edits, &[Some("i1".to_string())]);
        assert_eq!(out.len(), 1);
        assert_eq!(out.get("index.jsonl"), Some(&"i1".to_string()));
    }

    /// Writes land on `refs/heads/` whatever they were read from — a remote-tracking ref is
    /// a copy of someone else's branch, and moving it locally makes this clone lie about it.
    #[test]
    fn a_write_targets_the_local_branch() {
        assert_eq!(local_ref("trck-issues"), "refs/heads/trck-issues");
        assert_eq!(local_ref("origin/trck-issues"), "refs/heads/trck-issues");
        assert_eq!(local_ref("refs/heads/trck-issues"), "refs/heads/trck-issues");
    }

    /// git's own refusal is aimed at someone committing by hand and ends in "unable to
    /// auto-detect email address", which reads as a bug in trck.
    #[test]
    fn an_unset_identity_is_explained_with_the_config_to_set() {
        let raw = "git commit-tree: *** Please tell me who you are.\nfatal: unable to auto-detect email address".to_string();
        let msg = explain_identity(raw);
        assert!(msg.contains("user.email"), "{msg}");
        assert!(msg.contains("user.name"), "{msg}");
        assert!(!msg.contains("auto-detect"), "git's own wording is replaced, not appended: {msg}");
    }

    /// Every other failure is passed through untouched — swallowing one would hide the real
    /// diagnostic behind a guess about identity.
    #[test]
    fn an_unrelated_failure_is_left_alone() {
        assert_eq!(explain_identity("git write-tree: bad object".to_string()), "git write-tree: bad object");
    }
}

//! What a tracker holds, and how a verb gets at it.
//!
//! Split from [`super`] because finding a tracker and reading one are different jobs, and
//! only the second has to change when a tracker stops being a directory. The read verbs
//! call the content accessors here and never join a path themselves, so a second source —
//! a tracker resolved out of a git ref (`#sqzr7nk`) — lands as another arm in this file
//! rather than as an edit to nine verbs.
//!
//! The path accessors stay for the write side, which still puts bytes on a filesystem.

use super::{Ctx, ITEMS_DIR, Source};
use crate::issue::Issue;
use std::path::Path;

/// The two generated files, named once. A changeset addresses them by these names and the
/// path accessors below join the same ones, so a backend and a directory cannot disagree.
pub(crate) const INDEX_NAME: &str = "index.jsonl";
pub(crate) const SUMMARY_NAME: &str = "SUMMARY.md";

impl Ctx {
    /// Where to run git for this tracker.
    ///
    /// Either source answers: a directory-backed tracker is somewhere inside the repo it
    /// belongs to, and a ref-backed one carries the directory it was resolved from. So a
    /// caller that wants git — `diff`, resolving a revision — never has to know which it
    /// got.
    pub(crate) fn git_cwd(&self) -> &Path {
        match &self.source {
            Source::Dir(dir) => dir,
            Source::Ref { cwd, .. } => cwd,
        }
    }

    /// The tracker as a repo-relative prefix, the way `git show <rev>:<path>` wants it.
    ///
    /// `None` when the tracker is not inside a git repository at all; the caller decides
    /// what to say about that, because only it knows what the revision was wanted for.
    ///
    /// A ref-backed tracker is always the empty prefix: its root *is* the tracker, which is
    /// the whole point of putting it on a branch of its own. The path arithmetic is for the
    /// working-tree case, where `issues/` sits somewhere inside a repository of code.
    pub(crate) fn tracker_prefix(&self) -> Result<Option<String>, String> {
        let Ok(tracker) = self.dir() else {
            return Ok(Some(String::new()));
        };
        let Some(root) = crate::git::repo_root(tracker)? else {
            return Ok(None);
        };
        let dir = tracker.canonicalize().unwrap_or_else(|_| tracker.to_path_buf());
        let root = root.canonicalize().unwrap_or(root);
        let rel = dir.strip_prefix(&root).map_err(|_| format!("tracker dir {} is not inside the git repo at {}", tracker.display(), root.display()))?;
        let rel = rel.to_string_lossy().replace('\\', "/");
        Ok(Some(if rel.is_empty() || rel == "." { String::new() } else { format!("{rel}/") }))
    }

    /// How to name this tracker and the project around it, for a page title.
    ///
    /// A directory says where it is; a ref says which ref it is. Both are what someone
    /// would use to find it again, which is the only job the label has.
    pub(crate) fn labels(&self) -> (String, String) {
        let name = |p: Option<&std::ffi::OsStr>| p.map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        match &self.source {
            Source::Dir(dir) => (name(dir.parent().and_then(std::path::Path::file_name)), name(dir.file_name())),
            Source::Ref { rev, cwd } => (name(cwd.file_name()), rev.clone()),
        }
    }

    /// The raw `index.jsonl`.
    ///
    /// An absent index reads as empty rather than failing, from either source: `init`
    /// leaves exactly that state on disk, a revision from before the tracker existed is
    /// the same thing in a ref, and every read verb has to work on both.
    pub(crate) fn read_index(&self) -> Result<String, String> {
        match &self.source {
            Source::Dir(_) => Ok(std::fs::read_to_string(self.index_path()?).unwrap_or_default()),
            Source::Ref { rev, cwd } => Ok(crate::git::show(cwd, rev, INDEX_NAME)?.unwrap_or_default()),
        }
    }

    /// One issue's markdown body.
    ///
    /// The missing-body wording is contract: `show` and the mutating verbs both report a
    /// vanished body this way and the conformance suite compares it. It reads the same from
    /// either source — one broken tracker, one diagnostic — and only the location after the
    /// colon says which source it came from.
    pub(crate) fn read_body(&self, row: &Issue) -> Result<String, String> {
        let name = crate::summary::filename(row);
        match &self.source {
            Source::Dir(_) => {
                let path = self.items_dir()?.join(&name);
                if !path.exists() {
                    return Err(format!("file missing for #{}: {}", row.id, path.display()));
                }
                std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))
            },
            Source::Ref { rev, cwd } => {
                let at = format!("{ITEMS_DIR}/{name}");
                crate::git::show(cwd, rev, &at)?.ok_or_else(|| format!("file missing for #{}: {rev}:{at}", row.id))
            },
        }
    }

    /// Every name in the items directory, sorted, unfiltered.
    ///
    /// Unfiltered because deciding what counts as an issue file is `validate`'s rule and
    /// its diagnostics depend on seeing the rejects: a README parked in `items/` has to be
    /// visible to be ignored deliberately rather than invisibly.
    pub(crate) fn list_items(&self) -> Result<Vec<String>, String> {
        match &self.source {
            Source::Dir(_) => {
                let Ok(entries) = std::fs::read_dir(self.items_dir()?) else {
                    return Ok(Vec::new());
                };
                let mut names: Vec<String> = entries.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
                names.sort();
                Ok(names)
            },
            Source::Ref { rev, cwd } => Ok(crate::git::ls_tree(cwd, rev, ITEMS_DIR)?.unwrap_or_default()),
        }
    }
}

#[cfg(test)]
mod tests {
    // Tests assert; that is their job. The crate denies unwrap/expect/panic because a
    // malformed tracker must produce a diagnostic rather than a stack trace, but a test
    // that cannot panic cannot fail.
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;
    use crate::discovery::tests::Tmp;

    /// One row, parsed the way a real index is, so the test needs no constructor of its own.
    fn row(json: &str) -> crate::issue::Issue {
        crate::index::parse_index(json, "index.jsonl").expect("parses").pop().expect("one row")
    }

    #[test]
    fn read_index_answers_with_the_file_contents() {
        let tmp = Tmp::new("readindex");
        let d = tmp.tracker("issues");
        std::fs::write(d.join("index.jsonl"), "{\"id\": \"aaa1111\"}\n").expect("write");
        let ctx = Ctx::load(Source::Dir(d), false).expect("loads");
        assert_eq!(ctx.read_index().expect("read"), "{\"id\": \"aaa1111\"}\n");
    }

    /// A tracker whose index has not been written yet reads as empty rather than failing:
    /// `init` leaves exactly that state, and every read verb has to work on it.
    #[test]
    fn read_index_of_a_tracker_without_one_is_empty() {
        let tmp = Tmp::new("noindex");
        let ctx = Ctx::load(Source::Dir(tmp.tracker("issues")), false).expect("loads");
        assert_eq!(ctx.read_index().expect("read"), "");
    }

    #[test]
    fn read_body_answers_with_the_issue_markdown() {
        let tmp = Tmp::new("readbody");
        let d = tmp.tracker("issues");
        std::fs::create_dir_all(d.join(ITEMS_DIR)).expect("mkdir");
        std::fs::write(d.join(ITEMS_DIR).join("aaa1111-a-title.md"), "# a title\n").expect("write");
        let ctx = Ctx::load(Source::Dir(d), false).expect("loads");
        let r = row("{\"id\": \"aaa1111\", \"slug\": \"a-title\", \"title\": \"a title\", \"status\": \"backlog\", \"priority\": \"medium\"}");
        assert_eq!(ctx.read_body(&r).expect("read"), "# a title\n");
    }

    /// The wording is contract: `show` and the mutating verbs both report a vanished body
    /// this way, and the conformance suite compares it. Naming the io error instead would
    /// name the file but not the issue.
    #[test]
    fn read_body_names_the_issue_when_the_file_is_missing() {
        let tmp = Tmp::new("nobody");
        let ctx = Ctx::load(Source::Dir(tmp.tracker("issues")), false).expect("loads");
        let r = row("{\"id\": \"aaa1111\", \"slug\": \"a-title\", \"title\": \"a title\", \"status\": \"backlog\", \"priority\": \"medium\"}");
        let err = ctx.read_body(&r).expect_err("missing");
        assert!(err.starts_with("file missing for #aaa1111: "), "{err}");
        assert!(err.ends_with("aaa1111-a-title.md"), "{err}");
    }

    #[test]
    fn list_items_answers_with_the_item_filenames_sorted() {
        let tmp = Tmp::new("listitems");
        let d = tmp.tracker("issues");
        std::fs::create_dir_all(d.join(ITEMS_DIR)).expect("mkdir");
        for name in ["bbb2222-b.md", "aaa1111-a.md", "README.md"] {
            std::fs::write(d.join(ITEMS_DIR).join(name), "").expect("write");
        }
        let ctx = Ctx::load(Source::Dir(d), false).expect("loads");
        // Everything in the directory, unfiltered — deciding what is an issue file belongs
        // to `validate`, which is the only caller that has the rules for it.
        assert_eq!(ctx.list_items().expect("list"), vec!["README.md", "aaa1111-a.md", "bbb2222-b.md"]);
    }

    #[test]
    fn list_items_of_a_tracker_without_an_items_dir_is_empty() {
        let tmp = Tmp::new("noitems");
        let ctx = Ctx::load(Source::Dir(tmp.tracker("issues")), false).expect("loads");
        assert!(ctx.list_items().expect("list").is_empty());
    }
}

//! What a tracker holds, and how a verb gets at it.
//!
//! Split from [`super`] because finding a tracker and reading one are different jobs, and
//! only the second has to change when a tracker stops being a directory. The read verbs
//! call the content accessors here and never join a path themselves, so a second source —
//! a tracker resolved out of a git ref (`#sqzr7nk`) — lands as another arm in this file
//! rather than as an edit to nine verbs.
//!
//! The path accessors stay for the write side, which still puts bytes on a filesystem.

use super::{Ctx, ITEMS_DIR};
use crate::issue::Issue;
use std::path::PathBuf;

/// The two generated files, named once. A changeset addresses them by these names and the
/// path accessors below join the same ones, so a backend and a directory cannot disagree.
pub(crate) const INDEX_NAME: &str = "index.jsonl";
pub(crate) const SUMMARY_NAME: &str = "SUMMARY.md";

impl Ctx {
    pub(crate) fn index_path(&self) -> PathBuf {
        self.dir.join(INDEX_NAME)
    }

    pub(crate) fn items_dir(&self) -> PathBuf {
        self.dir.join(ITEMS_DIR)
    }

    pub(crate) fn summary_path(&self) -> PathBuf {
        self.dir.join(SUMMARY_NAME)
    }

    /// The raw `index.jsonl`.
    ///
    /// An absent index reads as empty rather than failing: `init` leaves exactly that
    /// state, and every read verb has to work on it. The `Result` is not for that case —
    /// it is the seam. A directory can only fail to answer by not existing, but a tracker
    /// read out of a git ref can fail for reasons worth a sentence, and the caller should
    /// already be shaped to pass one through.
    #[expect(clippy::unnecessary_wraps, reason = "the seam is the point; the ref-backed source makes it fallible")]
    pub(crate) fn read_index(&self) -> Result<String, String> {
        Ok(std::fs::read_to_string(self.index_path()).unwrap_or_default())
    }

    /// One issue's markdown body.
    ///
    /// The missing-file wording is contract: `show` and the mutating verbs both report a
    /// vanished body this way and the conformance suite compares it, so it belongs here
    /// rather than at each call site — passing the raw io error through would name the
    /// file but not the issue.
    pub(crate) fn read_body(&self, row: &Issue) -> Result<String, String> {
        let path = self.items_dir().join(crate::summary::filename(row));
        if !path.exists() {
            return Err(format!("file missing for #{}: {}", row.id, path.display()));
        }
        std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// Every name in the items directory, sorted, unfiltered.
    ///
    /// Unfiltered because deciding what counts as an issue file is `validate`'s rule and
    /// its diagnostics depend on seeing the rejects: a README parked in `items/` has to be
    /// visible to be ignored deliberately rather than invisibly.
    #[expect(clippy::unnecessary_wraps, reason = "the seam is the point; the ref-backed source makes it fallible")]
    pub(crate) fn list_items(&self) -> Result<Vec<String>, String> {
        let Ok(entries) = std::fs::read_dir(self.items_dir()) else {
            return Ok(Vec::new());
        };
        let mut names: Vec<String> = entries.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        names.sort();
        Ok(names)
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
        let ctx = Ctx::load(d, false).expect("loads");
        assert_eq!(ctx.read_index().expect("read"), "{\"id\": \"aaa1111\"}\n");
    }

    /// A tracker whose index has not been written yet reads as empty rather than failing:
    /// `init` leaves exactly that state, and every read verb has to work on it.
    #[test]
    fn read_index_of_a_tracker_without_one_is_empty() {
        let tmp = Tmp::new("noindex");
        let ctx = Ctx::load(tmp.tracker("issues"), false).expect("loads");
        assert_eq!(ctx.read_index().expect("read"), "");
    }

    #[test]
    fn read_body_answers_with_the_issue_markdown() {
        let tmp = Tmp::new("readbody");
        let d = tmp.tracker("issues");
        std::fs::create_dir_all(d.join(ITEMS_DIR)).expect("mkdir");
        std::fs::write(d.join(ITEMS_DIR).join("aaa1111-a-title.md"), "# a title\n").expect("write");
        let ctx = Ctx::load(d, false).expect("loads");
        let r = row("{\"id\": \"aaa1111\", \"slug\": \"a-title\", \"title\": \"a title\", \"status\": \"backlog\", \"priority\": \"medium\"}");
        assert_eq!(ctx.read_body(&r).expect("read"), "# a title\n");
    }

    /// The wording is contract: `show` and the mutating verbs both report a vanished body
    /// this way, and the conformance suite compares it. Naming the io error instead would
    /// name the file but not the issue.
    #[test]
    fn read_body_names_the_issue_when_the_file_is_missing() {
        let tmp = Tmp::new("nobody");
        let ctx = Ctx::load(tmp.tracker("issues"), false).expect("loads");
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
        let ctx = Ctx::load(d, false).expect("loads");
        // Everything in the directory, unfiltered — deciding what is an issue file belongs
        // to `validate`, which is the only caller that has the rules for it.
        assert_eq!(ctx.list_items().expect("list"), vec!["README.md", "aaa1111-a.md", "bbb2222-b.md"]);
    }

    #[test]
    fn list_items_of_a_tracker_without_an_items_dir_is_empty() {
        let tmp = Tmp::new("noitems");
        let ctx = Ctx::load(tmp.tracker("issues"), false).expect("loads");
        assert!(ctx.list_items().expect("list").is_empty());
    }
}

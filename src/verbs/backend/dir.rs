//! A tracker that is a directory: the changeset applied as file operations.
//!
//! This is the only place in the engine that writes a tracker's files. The ref-backed
//! counterpart is [`super::git`], and neither knows the other exists.

use super::super::write::write_atomic;
use super::{Changeset, Edit, Op};
use std::path::Path;

/// A tracker that is a directory on disk.
pub(crate) struct DirBackend<'a> {
    dir: &'a Path,
}

impl<'a> DirBackend<'a> {
    pub(crate) fn new(dir: &'a Path) -> Self {
        Self { dir }
    }

    /// Apply every edit in order.
    ///
    /// The `Op` is what the verb was asked to do; a directory has nowhere to record that, so
    /// this backend takes it and drops it. A commit-building backend is the one that needs
    /// it, and it needs it here — which is why it is a parameter rather than something the
    /// caller keeps to itself.
    pub(crate) fn apply(&self, cs: &Changeset, _op: &Op) -> Result<(), String> {
        for edit in &cs.edits {
            self.apply_one(edit)?;
        }
        Ok(())
    }

    fn apply_one(&self, edit: &Edit) -> Result<(), String> {
        match edit {
            Edit::Write { path, contents } => write_atomic(&self.dir.join(path), contents),
            Edit::Rename { from, to } => {
                let (from, to) = (self.dir.join(from), self.dir.join(to));
                std::fs::rename(&from, &to).map_err(|e| format!("{} -> {}: {e}", from.display(), to.display()))
            },
            Edit::Delete { path } => {
                let path = self.dir.join(path);
                std::fs::remove_file(&path).map_err(|e| format!("{}: {e}", path.display()))
            },
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
    use std::path::PathBuf;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("trck-backend-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("mkdir");
        dir
    }

    fn op() -> Op {
        Op::new("test")
    }

    /// The paths in a changeset are tracker-relative; resolving them against the tracker
    /// directory is this backend's job and nobody else's.
    #[test]
    fn a_write_lands_under_the_tracker_directory() {
        let dir = scratch("write");
        let cs = Changeset::new(Vec::new(), vec![Edit::Write { path: PathBuf::from("items/aaaaaaa-a.md"), contents: "# a\n".into() }]);
        DirBackend::new(&dir).apply(&cs, &op()).expect("applies");
        assert_eq!(std::fs::read_to_string(dir.join("items/aaaaaaa-a.md")).expect("read"), "# a\n");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_rename_moves_the_body_and_leaves_nothing_behind() {
        let dir = scratch("rename");
        std::fs::create_dir_all(dir.join("items")).expect("mkdir");
        std::fs::write(dir.join("items/aaaaaaa-old.md"), "# old\n").expect("write");
        let cs = Changeset::new(Vec::new(), vec![Edit::Rename { from: PathBuf::from("items/aaaaaaa-old.md"), to: PathBuf::from("items/aaaaaaa-new.md") }]);
        DirBackend::new(&dir).apply(&cs, &op()).expect("applies");
        assert!(!dir.join("items/aaaaaaa-old.md").exists(), "the old name must be gone");
        assert_eq!(std::fs::read_to_string(dir.join("items/aaaaaaa-new.md")).expect("read"), "# old\n");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_delete_removes_the_file() {
        let dir = scratch("delete");
        std::fs::write(dir.join("gone.md"), "x").expect("write");
        let cs = Changeset::new(Vec::new(), vec![Edit::Delete { path: PathBuf::from("gone.md") }]);
        DirBackend::new(&dir).apply(&cs, &op()).expect("applies");
        assert!(!dir.join("gone.md").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Order is the contract: `set --slug --title` renames the body and then rewrites its
    /// heading, and a write applied before the rename would land on the name that is about to
    /// move away.
    #[test]
    fn edits_are_applied_in_order() {
        let dir = scratch("order");
        std::fs::create_dir_all(dir.join("items")).expect("mkdir");
        std::fs::write(dir.join("items/aaaaaaa-old.md"), "# old\nbody\n").expect("write");
        let cs = Changeset::new(
            Vec::new(),
            vec![
                Edit::Rename { from: PathBuf::from("items/aaaaaaa-old.md"), to: PathBuf::from("items/aaaaaaa-new.md") },
                Edit::Write { path: PathBuf::from("items/aaaaaaa-new.md"), contents: "# new\nbody\n".into() },
            ],
        );
        DirBackend::new(&dir).apply(&cs, &op()).expect("applies");
        assert_eq!(std::fs::read_to_string(dir.join("items/aaaaaaa-new.md")).expect("read"), "# new\nbody\n");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A failure names the file. The write path reports a diagnostic rather than panicking,
    /// and an operation that says only "No such file" is one the user cannot act on.
    #[test]
    fn a_rename_of_a_missing_file_reports_both_names() {
        let dir = scratch("badrename");
        let cs = Changeset::new(Vec::new(), vec![Edit::Rename { from: PathBuf::from("nope.md"), to: PathBuf::from("also-nope.md") }]);
        let err = DirBackend::new(&dir).apply(&cs, &op()).expect_err("cannot rename what is not there");
        assert!(err.contains("nope.md") && err.contains("also-nope.md"), "{err}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

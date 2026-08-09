//! The one way this engine writes a file.
//!
//! Writes are **atomic**: a temporary file in the same directory, then a rename. A rename
//! within a directory is atomic on every platform trck runs on, so an interrupted run leaves
//! the previous contents rather than a truncated file. The index is the tracker's only source
//! of truth; half of one is worse than none.

use std::path::Path;

/// Write a file by writing a sibling temporary and renaming over the target.
pub(crate) fn write_file(path: &Path, contents: &str) -> Result<(), String> {
    write_atomic(path, contents)
}

pub(crate) fn write_atomic(path: &Path, contents: &str) -> Result<(), String> {
    // The pid keeps two trck processes in one tracker from racing on the same temporary.
    let tmp = path.with_extension(format!("tmp{}", std::process::id()));
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    std::fs::write(&tmp, contents).map_err(|e| format!("{}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    // Tests assert; that is their job. The crate denies unwrap/expect/panic because a
    // malformed tracker must produce a diagnostic rather than a stack trace, but a test
    // that cannot panic cannot fail.
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("trck-write-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    /// The parent directory is created rather than required: `new` writes into `items/` on a
    /// tracker that may not have one yet.
    #[test]
    fn a_missing_parent_directory_is_created() {
        let dir = scratch("mkdir");
        let path = dir.join("nested/deeper/file.txt");
        write_atomic(&path, "body").expect("writes");
        assert_eq!(std::fs::read_to_string(&path).expect("read"), "body");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Overwriting replaces the contents wholly — a shorter write must not leave a tail of
    /// the longer one behind, which is what a plain truncate-and-write can do on a crash.
    #[test]
    fn overwriting_leaves_no_tail_of_the_previous_contents() {
        let dir = scratch("overwrite");
        std::fs::create_dir_all(&dir).expect("mkdir");
        let path = dir.join("f.txt");
        write_atomic(&path, "a much longer previous body").expect("first");
        write_atomic(&path, "short").expect("second");
        assert_eq!(std::fs::read_to_string(&path).expect("read"), "short");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// No temporary is left behind on success; a stray `*.tmp<pid>` in `items/` would be read
    /// as a stray file by `check`.
    #[test]
    fn the_temporary_does_not_survive_a_successful_write() {
        let dir = scratch("notmp");
        std::fs::create_dir_all(&dir).expect("mkdir");
        write_atomic(&dir.join("f.txt"), "body").expect("writes");
        let left: Vec<String> =
            std::fs::read_dir(&dir).expect("read_dir").flatten().map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| n.contains("tmp")).collect();
        assert!(left.is_empty(), "temporaries left behind: {left:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

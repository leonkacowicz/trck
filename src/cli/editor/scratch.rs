//! The buffer file the editor is pointed at.
//!
//! Its own module because its whole job is a guarantee about cleanup, and a guarantee is
//! easier to keep when nothing else lives in the file.

use std::path::{Path, PathBuf};

/// A temp file that removes itself however the edit ends — accepted, aborted, or the editor
/// failing to start.
pub(super) struct Scratch(PathBuf);

impl Scratch {
    pub(super) fn new(seed: &str) -> Result<Scratch, String> {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static N: AtomicUsize = AtomicUsize::new(0);
        let n = N.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("trck-new-{}-{n}.md", std::process::id()));
        std::fs::write(&path, seed).map_err(|e| format!("new: {}: {e}", path.display()))?;
        Ok(Scratch(path))
    }

    pub(super) fn path(&self) -> &Path {
        &self.0
    }

    pub(super) fn read(&self) -> Result<String, String> {
        std::fs::read_to_string(&self.0).map_err(|e| format!("new: {}: {e}", self.0.display()))
    }

    pub(super) fn write(&self, text: &str) -> Result<(), String> {
        std::fs::write(&self.0, text).map_err(|e| format!("new: {}: {e}", self.0.display()))
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

#[cfg(test)]
mod tests {
    // Tests assert; that is their job. The crate denies unwrap/expect/panic because a
    // malformed tracker must produce a diagnostic rather than a stack trace, but a test
    // that cannot panic cannot fail.
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    #[test]
    fn the_scratch_file_is_removed_when_it_goes_out_of_scope() {
        let path = {
            let s = Scratch::new("seed").expect("scratch");
            assert!(s.path().is_file());
            s.path().to_path_buf()
        };
        assert!(!path.exists(), "scratch survived: {}", path.display());
    }
}

//! The throwaway tracker every test in this crate builds on.
//!
//! Its own file because it carries a rule, and a rule needs somewhere to be written down.
//!
//! **A fixture never puts `trck.json` at its own root.** [`Tmp::tracker`] takes a relative path
//! for exactly that reason, and it is not a stylistic preference: discovery walks up from a
//! start directory and, at *every* ancestor, scans that directory's **direct children** for a
//! `trck.json`. A fixture rooted at `<temp>/trck-test-x` with a config at its root therefore
//! makes `<temp>` a tracker to everything else running there — and the discovery tests that
//! assert there is *nothing* to find start finding it instead. One level further down, the
//! scan of `<temp>` never sees it.
//!
//! That is a race between suites, not a dirty-machine problem: `cargo test --all` runs test
//! binaries concurrently, and `tests/git_hooks.rs` builds a tracker at a repository root on
//! purpose. Both halves have to agree on this, which is why the same note appears there and in
//! `tests/new_body_stdin.rs`.
//!
//! `std::env::temp_dir` plus a counter rather than a crate — the engine takes no dependencies,
//! and its tests should not either.

// A fixture asserts; that is its job. The crate denies unwrap/expect/panic because a malformed
// tracker must produce a diagnostic rather than a stack trace, but a fixture that cannot panic
// cannot fail the test it is setting up for.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::CONFIG_NAME;
use std::path::{Path, PathBuf};

/// A throwaway directory tree, removed however the test ends.
///
/// [`Drop`] rather than a line at the end of the test body: a failing assertion unwinds past
/// that line, so a cleanup that only runs on success leaves a temp tree behind on exactly the
/// runs someone is about to repeat.
pub(crate) struct Tmp(pub(crate) PathBuf);

impl Tmp {
    pub(crate) fn new(tag: &str) -> Tmp {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static N: AtomicUsize = AtomicUsize::new(0);
        let n = N.fetch_add(1, Ordering::Relaxed);
        let p = std::env::temp_dir().join(format!("trck-test-{tag}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).expect("temp dir");
        Tmp(p)
    }

    /// The root of the throwaway tree, for a test that wants to place its own directories
    /// inside it rather than a ready-made tracker.
    pub(crate) fn path(&self) -> &Path {
        &self.0
    }

    /// A tracker at `rel`, **inside** the throwaway root.
    ///
    /// Relative by construction: see the note at the top of this file for why a tracker at the
    /// root itself would break every other test sharing the temp directory.
    pub(crate) fn tracker(&self, rel: &str) -> PathBuf {
        let d = self.0.join(rel);
        std::fs::create_dir_all(&d).expect("mkdir");
        std::fs::write(d.join(CONFIG_NAME), "{}").expect("write config");
        d.canonicalize().unwrap_or(d)
    }
}

impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

//! Finding the tracker directory.
//!
//! `trck` is meant to work from anywhere in a repo, so the directory is discovered
//! rather than passed. Resolution order is `--dir`, then `$TRCK_DIR`, then a vendored
//! engine's own directory, then walking up from the working directory — most explicit
//! first, so an override is never quietly ignored.

use crate::config::Config;
#[cfg(test)]
use crate::config::SUPPORTED_FORMAT;
use std::path::{Path, PathBuf};

pub(crate) mod content;
mod load;
mod source;
mod standing;

pub(crate) use source::{Overrides, Source, resolve_tracker_source};

/// The file whose presence marks a directory as a tracker.
pub(crate) const CONFIG_NAME: &str = "trck.json";

/// The one directory holding every issue body. Status lives in `index.jsonl` alone.
pub(crate) const ITEMS_DIR: &str = "items";

fn is_tracker(dir: &Path) -> bool {
    dir.join(CONFIG_NAME).is_file()
}

/// Walk up from `start` to the directory holding `trck.json`, or to one whose single
/// child holds it — so running from a repo root finds `issues/` without being told.
///
/// Two children holding one is ambiguous and refused rather than guessed at: picking
/// the alphabetically-first would silently write to the wrong tracker.
pub(crate) fn find_tracker(start: &Path) -> Result<PathBuf, String> {
    let mut cur = start.canonicalize().unwrap_or_else(|_| start.to_path_buf());
    loop {
        if is_tracker(&cur) {
            return Ok(cur);
        }
        let mut hits: Vec<PathBuf> = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&cur) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() && is_tracker(&path) {
                    hits.push(path);
                }
            }
        }
        hits.sort();
        match hits.len() {
            1 => {
                let hit = hits.remove(0);
                return Ok(hit.canonicalize().unwrap_or(hit));
            },
            0 => {},
            n => {
                return Err(format!("ambiguous tracker under {} ({n} found); pass --dir", cur.display()));
            },
        }
        match cur.parent() {
            Some(parent) if parent != cur => cur = parent.to_path_buf(),
            _ => return Err("no tracker found here; run `trck init`".to_string()),
        }
    }
}

/// A resolved invocation: where the tracker's bytes come from, and its config.
///
/// The source, not a directory. A tracker read out of a git ref has no directory at all,
/// so anything that genuinely needs one asks [`Ctx::dir`] and handles being told no — which
/// is every write verb and every `repo` verb, and none of the read verbs.
#[derive(Debug)]
pub(crate) struct Ctx {
    pub(crate) source: Source,
    pub(crate) config: Config,
}

/// Issue files still sitting in per-status folders, the pre-0.23 layout.
///
/// Status used to be encoded in the path and now lives only in `index.jsonl`, so such a
/// tracker has two sources of truth that can disagree. Every verb refuses one until
/// `repo migrate-layout` has run.
pub(crate) fn legacy_layout_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for status in crate::config::STATUSES {
        let folder = dir.join(status);
        let Ok(entries) = std::fs::read_dir(&folder) else {
            continue;
        };
        out.extend(entries.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "md")));
    }
    out.sort();
    out
}

/// The refusal an unmigrated tracker gets, naming the remedy.
fn check_layout(dir: &Path) -> Option<String> {
    let stale = legacy_layout_files(dir);
    if stale.is_empty() {
        return None;
    }
    let mut folders: Vec<String> = stale.iter().filter_map(|p| p.parent()?.file_name()).map(|f| format!("{}/", f.to_string_lossy())).collect();
    folders.sort();
    folders.dedup();
    Some(format!(
        "legacy status-folder layout: {} issue file(s) under {} — run `trck repo \
         migrate-layout` to move them into {ITEMS_DIR}/ (status now lives only in index.jsonl)",
        stale.len(),
        folders.join(", ")
    ))
}

#[cfg(test)]
pub(crate) mod tests {
    // Tests assert; that is their job. The crate denies unwrap/expect/panic because a
    // malformed tracker must produce a diagnostic rather than a stack trace, but a test
    // that cannot panic cannot fail.
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    /// A throwaway directory tree. `std::env::temp_dir` plus a counter rather than a
    /// crate — the engine takes no dependencies, and its tests should not either.
    pub(crate) struct Tmp(PathBuf);

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

        /// The root of the throwaway tree, for a test that wants to place its own
        /// directories inside it rather than a ready-made tracker.
        pub(crate) fn path(&self) -> &Path {
            &self.0
        }

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

    #[test]
    fn finds_a_tracker_in_the_current_directory() {
        let tmp = Tmp::new("here");
        let d = tmp.tracker("issues");
        assert_eq!(find_tracker(&d).expect("found"), d);
    }

    #[test]
    fn finds_a_tracker_as_a_child_of_an_ancestor() {
        // The common case: running from anywhere in a repo whose tracker is `issues/`.
        let tmp = Tmp::new("child");
        let d = tmp.tracker("issues");
        let deep = tmp.0.join("src/deep");
        std::fs::create_dir_all(&deep).expect("mkdir");
        assert_eq!(find_tracker(&deep).expect("found"), d);
    }

    #[test]
    fn two_candidate_children_are_ambiguous_not_a_guess() {
        let tmp = Tmp::new("ambig");
        tmp.tracker("issues");
        tmp.tracker("other");
        let err = find_tracker(&tmp.0).expect_err("ambiguous");
        assert!(err.contains("ambiguous tracker"), "{err}");
        assert!(err.contains("pass --dir"), "{err}");
    }

    #[test]
    fn no_tracker_anywhere_says_how_to_make_one() {
        let tmp = Tmp::new("none");
        let err = find_tracker(&tmp.0).expect_err("not found");
        assert_eq!(err, "no tracker found here; run `trck init`");
    }

    /// The binary's own location is no longer a source of trackers. It used to be — an
    /// engine committed beside the tracker it served resolved that one — and it stops
    /// making sense for something installed on the machine rather than in the repo.
    #[test]
    fn the_binarys_own_directory_is_not_a_tracker_source() {
        let tmp = Tmp::new("selfdir");
        // A tracker at <tmp>/beside/issues — where a binary living in `beside` would once
        // have found it — and a working directory in a different branch of the tree, so
        // neither walking up nor looking down can reach it. Nothing points at it, so
        // nothing finds it.
        tmp.tracker("beside/issues");
        let elsewhere = tmp.0.join("elsewhere");
        std::fs::create_dir_all(&elsewhere).expect("mkdir");
        let err = find_tracker(&elsewhere).expect_err("no tracker in reach");
        assert!(err.contains("no tracker found"), "{err}");
    }

    #[test]
    fn loading_applies_the_format_guard_and_can_skip_it() {
        let tmp = Tmp::new("guard");
        let d = tmp.tracker("issues");
        std::fs::write(d.join(CONFIG_NAME), r#"{"format": 99}"#).expect("write");
        let err = Ctx::load(Source::Dir(d.clone()), true).expect_err("refused");
        assert!(err.contains("newer than this engine"), "{err}");
        // `update` is the remedy the refusal names, so it must survive the thing it fixes.
        assert!(Ctx::load(Source::Dir(d), false).is_ok());
    }

    /// Load every `trck.json` committed in this repo.
    ///
    /// The unit tests above cover shapes someone thought to write down; this covers the
    /// ones actually in use, and would catch a guard that rejects a real tracker.
    #[test]
    fn the_repos_own_configs_load_and_pass_the_guard() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf();
        let mut checked = 0;
        for rel in ["issues", "examples/action-game"] {
            let dir = root.join(rel);
            if !dir.join(CONFIG_NAME).is_file() {
                continue; // a consumer of this crate need not have the tracker
            }
            let ctx = Ctx::load(Source::Dir(dir), true).unwrap_or_else(|e| panic!("{rel}: {e}"));
            assert_eq!(ctx.config.format, Some(SUPPORTED_FORMAT), "{rel}");
            assert!(crate::config::vestigial_warnings(&ctx.config).is_empty(), "{rel} still carries a vocabulary key");
            checked += 1;
        }
        assert!(checked > 0, "no committed tracker found to check");
    }

    #[test]
    fn a_missing_config_file_loads_as_empty() {
        // Discovery guarantees the file exists, but a race or a half-made tracker
        // should give defaults rather than a panic.
        let tmp = Tmp::new("missing");
        let ctx = Ctx::load(Source::Dir(tmp.0.clone()), true).expect("loads");
        assert_eq!(ctx.config.update_repo(), crate::config::DEFAULT_UPDATE_REPO);
    }
}

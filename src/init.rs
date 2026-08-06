//! `init` — scaffold a tracker into a repository.
//!
//! The one verb that runs *without* a tracker, so it takes its target rather than
//! discovering one, and it is the only place the engine writes a `trck.json` from scratch.
//!
//! It no longer vendors. The Python engine copied itself into the tracker dir by default,
//! and that was the right answer while the engine was a script: a committed copy is pinned
//! to the version the data expects, it works in CI with nothing installed, and it cannot
//! drift. None of that survives the move to a binary — a copy is one platform's executable,
//! useless to the next contributor and wrong to commit — and the thing vendoring was really
//! standing in for, "refuse data this engine does not understand", is now the format guard's
//! job. So the tracker gets no engine, and the answer to "which trck runs this?" is whichever
//! one is installed.

use crate::config::{DEFAULT_UPDATE_REPO, SUPPORTED_FORMAT};
use crate::json::Json;
use crate::verbs::write_atomic;
use std::path::{Path, PathBuf};

/// The scaffolded docs, compiled in for the same reason the HTML assets are: a binary that
/// needs a file next to it is not one artifact.
const SCAFFOLD_CLAUDE_MD: &str = include_str!("../assets/scaffold-CLAUDE.md");
const SCAFFOLD_README_MD: &str = include_str!("../assets/scaffold-README.md");

/// What `init` was asked to do. Resolved by the CLI so this stays testable without argv.
pub(crate) struct InitOpts {
    /// Where to put the tracker. `None` means `issues/` under the working directory.
    pub(crate) target: Option<PathBuf>,
    /// Overwrite `trck.json` and the scaffolded docs of an existing tracker.
    pub(crate) force: bool,
    /// Also install the pre-commit consistency hook.
    pub(crate) hook: bool,
}

/// `trck.json` as a fresh tracker gets it: the format version, and where updates come from.
fn default_config() -> Json {
    Json::Object(vec![
        ("format".to_string(), Json::Number(SUPPORTED_FORMAT.to_string())),
        (
            "update".to_string(),
            Json::Object(vec![
                ("repo".to_string(), Json::String(DEFAULT_UPDATE_REPO.to_string())),
                ("channel".to_string(), Json::String("stable".to_string())),
            ]),
        ),
    ])
}

/// Write a scaffolded doc, leaving a customised one alone unless forced.
///
/// The asymmetry with `trck.json` is deliberate: the config is ours and reproducible, these
/// are documents somebody may have edited, and silently reverting an edit is worse than
/// leaving a stale sentence.
fn scaffold_doc(dir: &Path, name: &str, body: &str, force: bool) -> Result<(), String> {
    let path = dir.join(name);
    if force || !path.exists() {
        write_atomic(&path, body)?;
    }
    Ok(())
}

/// `init [dir] [--force] [--hook]` — create a tracker and its scaffolded docs.
pub(crate) fn cmd_init(opts: &InitOpts) -> Result<String, String> {
    let cwd = std::env::current_dir().map_err(|e| format!("cannot read the working directory: {e}"))?;
    let target = match &opts.target {
        Some(t) if t.is_absolute() => t.clone(),
        Some(t) => cwd.join(t),
        None => cwd.join("issues"),
    };

    let cfgfile = target.join("trck.json");
    if cfgfile.exists() && !opts.force {
        return Err(format!("{} is already a tracker (pass --force to overwrite config)", target.display()));
    }
    std::fs::create_dir_all(&target).map_err(|e| format!("{}: {e}", target.display()))?;

    write_atomic(&cfgfile, &(default_config().to_json_pretty() + "\n"))?;
    scaffold_doc(&target, "CLAUDE.md", SCAFFOLD_CLAUDE_MD, opts.force)?;
    scaffold_doc(&target, "README.md", SCAFFOLD_README_MD, opts.force)?;

    let mut out = Vec::new();
    if opts.hook {
        // Reported before the headline: the hook is a side effect the user asked for, and
        // burying its path under "initialized" would make a failure to find it puzzling.
        let ctx = crate::discovery::Ctx::load(target.clone(), false)?;
        out.push(crate::repo::cmd_install_hook(&ctx)?);
    }
    out.push(format!("initialized tracker at {}", target.display()));
    Ok(out.join("\n"))
}

#[cfg(test)]
mod tests {
    // Tests assert; that is their job. The crate denies unwrap/expect/panic because a
    // malformed tracker must produce a diagnostic rather than a stack trace, but a test
    // that cannot panic cannot fail.
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;
    use crate::discovery::tests::Tmp;

    fn init_at(dir: &Path, force: bool) -> Result<String, String> {
        cmd_init(&InitOpts { target: Some(dir.to_path_buf()), force, hook: false })
    }

    #[test]
    fn a_fresh_tracker_gets_a_config_and_both_docs() {
        let tmp = Tmp::new("init-fresh");
        let dir = tmp.path().join("issues");
        init_at(&dir, false).expect("initialises");
        for name in ["trck.json", "CLAUDE.md", "README.md"] {
            assert!(dir.join(name).exists(), "{name} not written");
        }
    }

    #[test]
    fn the_config_names_the_format_this_engine_understands() {
        let tmp = Tmp::new("init-config");
        let dir = tmp.path().join("issues");
        init_at(&dir, false).expect("initialises");
        let text = std::fs::read_to_string(dir.join("trck.json")).expect("readable");
        assert!(text.contains(&format!("\"format\": {SUPPORTED_FORMAT}")), "{text}");
        assert!(text.contains(DEFAULT_UPDATE_REPO), "{text}");
        assert!(text.ends_with('\n'), "config not newline-terminated: {text:?}");
    }

    /// The whole point of the verb changing hands. A copy of a binary is one platform's
    /// executable; committing it would be worse than useless to the next contributor.
    #[test]
    fn no_engine_is_copied_into_the_tracker() {
        let tmp = Tmp::new("init-novendor");
        let dir = tmp.path().join("issues");
        init_at(&dir, false).expect("initialises");
        assert!(!dir.join("trck").exists(), "init vendored an engine");
    }

    #[test]
    fn an_existing_tracker_is_refused_and_names_the_way_through() {
        let tmp = Tmp::new("init-exists");
        let dir = tmp.path().join("issues");
        init_at(&dir, false).expect("initialises");
        let err = init_at(&dir, false).expect_err("refuses");
        assert!(err.contains("already a tracker"), "{err}");
        assert!(err.contains("--force"), "{err}");
    }

    #[test]
    fn force_rewrites_the_config() {
        let tmp = Tmp::new("init-force");
        let dir = tmp.path().join("issues");
        init_at(&dir, false).expect("initialises");
        std::fs::write(dir.join("trck.json"), "{}\n").expect("writable");
        init_at(&dir, true).expect("forces");
        let text = std::fs::read_to_string(dir.join("trck.json")).expect("readable");
        assert!(text.contains("\"format\""), "config not rewritten: {text}");
    }

    /// A doc somebody edited is theirs. Reverting it silently on a re-init would lose work
    /// with no diagnostic, which is the one outcome worth more than being up to date.
    #[test]
    fn a_customised_doc_survives_a_re_init_unless_forced() {
        let tmp = Tmp::new("init-docs");
        let dir = tmp.path().join("issues");
        init_at(&dir, false).expect("initialises");
        std::fs::write(dir.join("CLAUDE.md"), "mine\n").expect("writable");
        init_at(&dir, true).expect("forces config");
        assert_eq!(std::fs::read_to_string(dir.join("CLAUDE.md")).expect("readable"), SCAFFOLD_CLAUDE_MD, "--force should refresh the docs too");

        let other = tmp.path().join("second");
        init_at(&other, false).expect("initialises");
        std::fs::write(other.join("CLAUDE.md"), "mine\n").expect("writable");
        // Not forced: the edit stands.
        let err = init_at(&other, false).expect_err("refuses without force");
        assert!(err.contains("already a tracker"), "{err}");
        assert_eq!(std::fs::read_to_string(other.join("CLAUDE.md")).expect("readable"), "mine\n");
    }

    #[test]
    fn the_scaffolded_docs_are_not_empty() {
        assert!(SCAFFOLD_CLAUDE_MD.contains("trck"), "CLAUDE.md scaffold");
        assert!(SCAFFOLD_README_MD.contains("trck"), "README scaffold");
        // The binary is not copyable, so the scaffold must not tell anyone to run a local one.
        assert!(!SCAFFOLD_README_MD.contains("vendored"), "README still mentions vendoring");
    }
}

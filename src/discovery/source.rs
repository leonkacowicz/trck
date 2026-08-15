//! Which tracker an invocation means, when it is not a directory in the working tree.
//!
//! Split from [`super`] because walking up a filesystem and asking git to resolve a ref are
//! different questions with different failure modes, and only the second one grows: the
//! local-versus-remote rule and the staleness report both land here later.

use super::{CONFIG_NAME, find_tracker, is_tracker};
use std::path::{Path, PathBuf};

/// Where the engine should look, given the explicit overrides.
///
/// `dir_opt` is `--dir` and `env_dir` is `$TRCK_DIR`; absent both, walk up from `cwd`. An
/// explicit override that is not a tracker is an error rather than a fallback, because
/// silently walking up from a mistyped `--dir` is how you edit the wrong repo.
///
/// There was a third source: the directory holding the running binary, which resolved a
/// tracker with an engine committed beside it. That made sense while the engine was a file
/// a repository could vendor. It is a binary now — installed on the machine, never in the
/// repo it serves — so its own location says nothing about which tracker anyone means.
pub(crate) fn resolve_tracker_dir(dir_opt: Option<&str>, env_dir: Option<&str>, cwd: &Path) -> Result<PathBuf, String> {
    if let Some(explicit) = dir_opt.or(env_dir) {
        let p = Path::new(explicit);
        let p = p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
        if !is_tracker(&p) {
            return Err(format!("{} is not a tracker (no {CONFIG_NAME})", p.display()));
        }
        return Ok(p);
    }
    find_tracker(cwd)
}

/// The branch a ref-backed tracker lives on when nobody says otherwise.
///
/// Convention rather than configuration, and deliberately not `refs/trck/issues`: a ref
/// outside `refs/heads/` is not in the default fetch refspec, so a fresh clone would find
/// nothing and read it as an *empty tracker* rather than as an error. Hyphen rather than
/// `trck/issues`, because git cannot hold both `refs/heads/trck` and `refs/heads/trck/…`
/// and `trck` is a branch name this repository would plausibly want.
pub(crate) const TRACKER_REF: &str = "trck-issues";

/// Where a tracker's bytes come from.
///
/// A directory is the only kind that exists on disk; a ref is read out of the object store
/// with no checkout, which is what lets a read answer the same thing from any branch and
/// any working tree state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Source {
    Dir(PathBuf),
    /// A revision git can resolve — `trck-issues`, `origin/trck-issues`, or whatever
    /// `--ref` named.
    ///
    /// `cwd` travels with it because reading the ref means running git, and git has to be
    /// run from inside the repository that holds it. Carrying it here keeps the source
    /// self-readable instead of making every caller remember to pass a directory
    /// alongside.
    Ref {
        rev: String,
        cwd: PathBuf,
    },
}

/// What the invocation said, before discovery gets a turn.
///
/// A struct rather than four parameters because the flag-beats-environment rule is part of
/// the resolution order and belongs beside the rest of it, not spread across call sites
/// that each have to remember which way round it goes.
#[derive(Debug, Default)]
pub(crate) struct Overrides<'a> {
    /// `--dir`
    pub(crate) dir: Option<&'a str>,
    /// `$TRCK_DIR`
    pub(crate) env_dir: Option<&'a str>,
    /// `--ref`
    pub(crate) git_ref: Option<&'a str>,
    /// `$TRCK_REF`
    pub(crate) env_ref: Option<&'a str>,
}

/// Which tracker the invocation means, directory or ref.
///
/// The order is `--dir` → `$TRCK_DIR` → `--ref` → `$TRCK_REF` → the working-tree walk-up →
/// [`TRACKER_REF`]. Most explicit first, so an override is never quietly ignored, and an
/// explicit one that does not resolve is an error rather than a fallback: silently walking
/// up from a mistyped `--ref` is the same way you edit the wrong repo that `--dir` already
/// guards against.
///
/// **A working-tree tracker beats the conventional ref.** That is what lets the move to a
/// ref-backed tracker land in stages — a checkout keeps behaving exactly as it did until
/// its `issues/` directory actually goes away.
pub(crate) fn resolve_tracker_source(over: &Overrides, cwd: &Path) -> Result<Source, String> {
    if over.dir.or(over.env_dir).is_some() {
        return resolve_tracker_dir(over.dir, over.env_dir, cwd).map(Source::Dir);
    }
    if let Some(explicit) = over.git_ref.or(over.env_ref) {
        return resolve_ref(cwd, explicit)?.ok_or_else(|| format!("'{explicit}' is not a resolvable git ref"));
    }
    match find_tracker(cwd) {
        Ok(dir) => Ok(Source::Dir(dir)),
        // The walk-up's wording is what someone who has simply not made a tracker yet
        // should read, so it survives whenever the ref is not there either.
        Err(not_found) => conventional_ref(cwd)?.ok_or(not_found),
    }
}

/// `rev` as a source, or `None` when git cannot resolve it.
///
/// A git that will not spawn is an error rather than a `None`: "there is no such ref" and
/// "this machine has no git" want different sentences, and only the first one should ever
/// read as "you typed the name wrong".
fn resolve_ref(cwd: &Path, rev: &str) -> Result<Option<Source>, String> {
    Ok(crate::git::rev_parse(cwd, rev)?.map(|_| Source::Ref { rev: rev.to_string(), cwd: cwd.to_path_buf() }))
}

/// [`TRACKER_REF`] as git resolves it here: the local branch, else the remote-tracking one.
///
/// Both are tried by name because a bare `trck-issues` does not reach
/// `refs/remotes/origin/trck-issues` — git's revision lookup would want
/// `refs/remotes/trck-issues`. So a fresh clone, which has only the remote-tracking ref,
/// needs the second attempt. Which of the two wins once both exist, and what to say when
/// they have diverged, is [`super::standing`].
fn conventional_ref(cwd: &Path) -> Result<Option<Source>, String> {
    let local = crate::git::rev_parse(cwd, TRACKER_REF)?;
    let remote_name = format!("origin/{TRACKER_REF}");
    let remote = crate::git::rev_parse(cwd, &remote_name)?;

    let (Some(local_sha), Some(remote_sha)) = (local.as_deref(), remote.as_deref()) else {
        // Only one of them exists, or neither. A fresh clone has just the remote-tracking
        // ref; a tracker made locally and never pushed has just the branch.
        let only = local.map(|_| TRACKER_REF.to_string()).or(remote.map(|_| remote_name));
        return Ok(only.map(|rev| Source::Ref { rev, cwd: cwd.to_path_buf() }));
    };

    // Local answers in every remaining case. What differs is what has to happen first.
    super::standing::reconcile(cwd, local_sha, remote_sha)?;
    Ok(Some(Source::Ref { rev: TRACKER_REF.to_string(), cwd: cwd.to_path_buf() }))
}

#[cfg(test)]
mod tests {
    // Tests assert; that is their job. The crate denies unwrap/expect/panic because a
    // malformed tracker must produce a diagnostic rather than a stack trace, but a test
    // that cannot panic cannot fail.
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;
    use crate::discovery::tests::Tmp;

    /// Resolution with git in play is covered end to end by `tests/ref_tracker.rs`, which
    /// has a real repository to resolve against. What is worth asserting here is the
    /// *order*, which holds whether or not any ref exists.
    fn over<'a>(dir: Option<&'a str>, env_dir: Option<&'a str>, git_ref: Option<&'a str>, env_ref: Option<&'a str>) -> Overrides<'a> {
        Overrides { dir, env_dir, git_ref, env_ref }
    }

    #[test]
    fn dir_wins_over_env_and_discovery() {
        let tmp = Tmp::new("order");
        let a = tmp.tracker("a");
        let b = tmp.tracker("b");
        let got = resolve_tracker_dir(Some(&a.display().to_string()), Some(&b.display().to_string()), tmp.path()).expect("resolved");
        assert_eq!(got, a);
    }

    #[test]
    fn env_wins_over_discovery() {
        let tmp = Tmp::new("env");
        let b = tmp.tracker("b");
        let got = resolve_tracker_dir(None, Some(&b.display().to_string()), tmp.path()).expect("resolved");
        assert_eq!(got, b);
    }

    #[test]
    fn an_explicit_override_that_is_not_a_tracker_is_an_error() {
        // Not a fallback: silently walking up from a mistyped --dir is how you edit the
        // wrong repo.
        let tmp = Tmp::new("bogus");
        tmp.tracker("issues");
        let err = resolve_tracker_dir(Some(&tmp.path().display().to_string()), None, tmp.path()).expect_err("refused");
        assert!(err.contains("is not a tracker"), "{err}");
    }

    /// `--dir` and `--ref` name different kinds of tracker, so one has to win. The
    /// directory does, matching the order the whole epic lands in: nothing about a
    /// checkout's behaviour changes until its `issues/` actually goes away.
    #[test]
    fn dir_wins_over_ref() {
        let tmp = Tmp::new("dirvsref");
        let a = tmp.tracker("a");
        let spec = a.display().to_string();
        let got = resolve_tracker_source(&over(Some(&spec), None, Some(TRACKER_REF), None), tmp.path()).expect("resolved");
        assert_eq!(got, Source::Dir(a));
    }

    /// An explicit `--ref` is not a hint. Falling back to the walk-up would resolve the
    /// tracker sitting right there and act on it silently — the same failure `--dir`
    /// already refuses.
    #[test]
    fn an_explicit_ref_does_not_fall_back_to_the_walk_up() {
        let tmp = Tmp::new("refnofall");
        tmp.tracker("issues");
        let err = resolve_tracker_source(&over(None, None, Some("no-such-ref"), None), tmp.path()).expect_err("refused");
        assert!(err.contains("no-such-ref"), "the refusal must name the ref: {err}");
    }

    #[test]
    fn the_ref_flag_beats_the_ref_env_var() {
        let tmp = Tmp::new("refenv");
        let err = resolve_tracker_source(&over(None, None, Some("from-flag"), Some("from-env")), tmp.path()).expect_err("refused");
        assert!(err.contains("from-flag"), "{err}");
        assert!(!err.contains("from-env"), "{err}");
    }

    #[test]
    fn the_ref_env_var_is_used_when_the_flag_is_absent() {
        let tmp = Tmp::new("refenvonly");
        let err = resolve_tracker_source(&over(None, None, None, Some("from-env")), tmp.path()).expect_err("refused");
        assert!(err.contains("from-env"), "{err}");
    }

    /// Outside a repository there is no ref to find, so the wording someone gets when they
    /// have simply not made a tracker yet must be exactly what it always was.
    #[test]
    fn no_tracker_and_no_ref_keeps_the_original_diagnostic() {
        let tmp = Tmp::new("nonenone");
        let err = resolve_tracker_source(&Overrides::default(), tmp.path()).expect_err("not found");
        assert_eq!(err, "no tracker found here; run `trck init`");
    }
}

//! Why a clone cannot see a tracker branch that exists.
//!
//! A default clone fetches `+refs/heads/*:refs/remotes/origin/*`, so `origin/trck-issues`
//! arrives with every ordinary fetch and stays fresh for free. A clone made with
//! `--single-branch` or `--depth` — which is what `actions/checkout` does by default —
//! narrows that to one branch, and then the tracker ref does not exist and never will,
//! however many times you fetch.
//!
//! Discovery's answer there would be *no tracker found*: honest about what it saw and wrong
//! about what is true. There is a tracker; this clone simply cannot see it, and the remedy
//! the message implies — make one — would make things worse.
//!
//! Telling the two apart means asking the remote, which costs a round trip. So nothing here
//! runs until discovery has already failed: the fast path is a ref that resolves.

use std::path::Path;

/// The refspec that brings the tracker branch in.
pub(crate) fn tracker_refspec(branch: &str) -> String {
    format!("+refs/heads/{branch}:refs/remotes/origin/{branch}")
}

/// Does any of these refspecs bring `branch` in?
///
/// Left-hand sides only, and only the two shapes that actually occur: the wildcard a default
/// clone gets, and an exact name someone added. A general refspec matcher would be a lot of
/// code for patterns git itself does not encourage — and being wrong in the *permissive*
/// direction here means saying "already covered" about a clone that cannot see the branch,
/// which is the failure this whole module exists to prevent.
pub(crate) fn covered(refspecs: &[String], branch: &str) -> bool {
    refspecs.iter().filter_map(|s| s.split(':').next()).any(|lhs| {
        let lhs = lhs.trim_start_matches('+');
        lhs == "refs/heads/*" || lhs == format!("refs/heads/{branch}") || lhs == "*"
    })
}

/// What this clone is configured to fetch.
pub(crate) fn configured(cwd: &Path, remote: &str) -> Vec<String> {
    crate::git::stdout(cwd, &["config", "--get-all", &format!("remote.{remote}.fetch")])
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect()
}

/// Does `remote` have a branch called `branch`?
///
/// `None` when the question could not be asked — no remote, no network, no git. That is not
/// the same as "no", and the caller must not report it as one: offline, the honest answer is
/// the one discovery already had.
pub(crate) fn remote_has_branch(cwd: &Path, remote: &str, branch: &str) -> Option<bool> {
    let out = crate::git::stdout(cwd, &["ls-remote", "--heads", remote, &format!("refs/heads/{branch}")]).ok()?;
    Some(!out.trim().is_empty())
}

/// The diagnostic for a clone whose refspec hides a tracker that is really there.
pub(crate) fn hidden(branch: &str, refspecs: &[String]) -> String {
    let listed = if refspecs.is_empty() { "(none configured)".to_string() } else { refspecs.join("\n    ") };
    format!(
        "the remote has a `{branch}` branch, but this clone does not fetch it — so the tracker \
         is there and invisible here, and no number of fetches will change that.\n  \
         this clone fetches:\n    {listed}\n  \
         run `trck repo setup-git` to add `{}`, then `git fetch`",
        tracker_refspec(branch)
    )
}

/// The refspec diagnostic, when that is what is really wrong.
///
/// `None` whenever the answer is not certain — no remote, no network, no branch there. Only
/// a remote that *has* the branch earns a different message; anything else keeps the wording
/// someone who has simply not made a tracker should read.
///
/// This runs on the error path only. Asking the remote costs a round trip, and by here
/// discovery has already failed.
pub(crate) fn why_invisible(cwd: &Path, branch: &str) -> Option<String> {
    let refspecs = configured(cwd, "origin");
    if covered(&refspecs, branch) {
        return None; // it would have arrived; its absence means it does not exist
    }
    remote_has_branch(cwd, "origin", branch)?.then(|| hidden(branch, &refspecs))
}

#[cfg(test)]
mod tests {
    // Tests assert; that is their job. The crate denies unwrap/expect/panic because a
    // malformed tracker must produce a diagnostic rather than a stack trace, but a test
    // that cannot panic cannot fail.
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    fn specs(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn a_default_clones_wildcard_covers_everything() {
        assert!(covered(&specs(&["+refs/heads/*:refs/remotes/origin/*"]), "trck-issues"));
    }

    /// What `--single-branch` and `actions/checkout` leave behind.
    #[test]
    fn a_single_branch_refspec_does_not_cover_the_tracker() {
        assert!(!covered(&specs(&["+refs/heads/main:refs/remotes/origin/main"]), "trck-issues"));
    }

    #[test]
    fn an_exact_refspec_for_the_branch_covers_it() {
        assert!(covered(&specs(&[&tracker_refspec("trck-issues")]), "trck-issues"));
    }

    /// Several refspecs is the state this leaves a clone in, and the second run must see the
    /// first one's work.
    #[test]
    fn one_covering_refspec_among_several_is_enough() {
        let v = specs(&["+refs/heads/main:refs/remotes/origin/main", "+refs/heads/trck-issues:refs/remotes/origin/trck-issues"]);
        assert!(covered(&v, "trck-issues"));
    }

    #[test]
    fn no_refspec_at_all_covers_nothing() {
        assert!(!covered(&[], "trck-issues"));
    }

    /// A refspec for a *different* branch of the same shape must not read as covering.
    #[test]
    fn a_refspec_for_another_branch_does_not_cover_this_one() {
        assert!(!covered(&specs(&["+refs/heads/trck-issues-old:refs/remotes/origin/trck-issues-old"]), "trck-issues"));
    }

    #[test]
    fn the_diagnostic_names_the_refspec_it_would_add_and_what_is_configured() {
        let msg = hidden("trck-issues", &specs(&["+refs/heads/main:refs/remotes/origin/main"]));
        assert!(msg.contains("+refs/heads/trck-issues:refs/remotes/origin/trck-issues"), "{msg}");
        assert!(msg.contains("+refs/heads/main:refs/remotes/origin/main"), "{msg}");
        assert!(msg.contains("trck repo setup-git"), "{msg}");
        // The thing it must never say, because it is what sent the reader wrong.
        assert!(!msg.contains("trck init"), "{msg}");
    }

    #[test]
    fn the_diagnostic_survives_a_clone_with_no_refspec_configured() {
        assert!(hidden("trck-issues", &[]).contains("(none configured)"));
    }
}

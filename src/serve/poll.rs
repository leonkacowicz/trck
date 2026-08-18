//! Keeping the served ref from going stale.
//!
//! **This is the one place in the engine that fetches on its own.** The standing rule is that
//! reads do not — a read that needs the network is a read that fails on a plane — and it is
//! stated where it belongs, in [`crate::cli::sync`]. The exception is deliberate and it is
//! about *shape*, not about convenience: that rule is written for a verb in a pipeline, where
//! a round trip is paid by every `trck list` anybody types. `serve` is one long-lived process
//! with a timer, so the network cost is paid once per interval no matter how many pages are
//! open, and the thing it buys is the only reason the verb exists. A tab left open on a
//! week-old ref is the time-travel bug in a new costume.
//!
//! Nothing here decides anything about which ref to read. The four-way rule lives in
//! [`crate::discovery::standing`] and is the same rule every read verb applies; this loop
//! supplies the one input a one-shot verb never has — that the remote may have moved since
//! the process started — and reports what the rule then did.

use crate::discovery::standing::{self, Resolution};
use crate::discovery::{Source, TRACKER_REF};
use std::path::Path;
use std::time::Duration;

/// The remote a tracker branch is shared through — the same convention the write path uses.
const REMOTE: &str = "origin";

/// How often the ref is refreshed when nobody says otherwise.
///
/// A tracker is not a stock ticker: the thing being watched changes when a person types a
/// verb, so anything under a few seconds is spending network on nothing. Thirty is short
/// enough that a page in another window is current by the time you look back at it, and long
/// enough that a laptop on a phone tether does not notice.
pub(crate) const DEFAULT_INTERVAL: Duration = Duration::from_secs(30);

/// The `--poll` argument as an interval, or the sentence that says what one is.
///
/// Zero is off rather than a busy loop: a tracker with no remote, or a machine deliberately
/// offline, has nothing for the timer to discover, and "never" has to be sayable.
pub(crate) fn interval_from(spec: Option<&str>) -> Result<Option<Duration>, String> {
    let Some(text) = spec else {
        return Ok(Some(DEFAULT_INTERVAL));
    };
    let secs: u64 = text.parse().map_err(|_| format!("bad poll interval '{text}' (must be a whole number of seconds; 0 turns polling off)"))?;
    Ok((secs > 0).then(|| Duration::from_secs(secs)))
}

/// What one tick concluded, in the vocabulary its log line uses.
///
/// A tick that found nothing says nothing, which is what makes the ones that do speak worth
/// reading. Everything here is compared against the previous tick's answer, so a state that
/// persists is reported when it *arrives* and not once per interval.
#[derive(PartialEq, Eq)]
enum Note {
    /// The remote is unreachable. Serving whatever is local, which is the whole point of
    /// preferring the local ref in the first place.
    Offline(String),
    /// The served ref now holds a different commit.
    Moved(String),
    /// Both sides moved. Never resolved here — local wins and this says so.
    Diverged,
    /// Behind, and the branch is checked out somewhere, so the fast-forward is refused. The
    /// page is serving the older commits and there is nothing this process may do about it.
    Blocked,
}

impl Note {
    /// What to print, once, when this note is new.
    fn line(&self) -> String {
        match self {
            Note::Offline(why) => format!("warning: cannot reach {REMOTE} ({why}); serving the local {TRACKER_REF} until it comes back"),
            Note::Moved(sha) => format!("note: {TRACKER_REF} is now {sha}; reload for the current tracker"),
            Note::Diverged => format!("warning: {}", standing::divergence()),
            Note::Blocked => format!(
                "warning: {TRACKER_REF} is behind {REMOTE}/{TRACKER_REF} and checked out in a worktree, so it cannot be \
                 fast-forwarded; this page is serving the older commits — detach that worktree, or run `trck sync`"
            ),
        }
    }
}

/// One pass: fetch, then let the rule say what that changed.
///
/// A fetch failure is a note, not a return: the local ref is still readable and still the
/// right thing to serve, so the loop goes on to look at it. That is criterion and design at
/// once — the process must not die because a laptop left the office.
fn tick(cwd: &Path, served: &str, previous: Option<&str>) -> (Vec<Note>, Option<String>) {
    let mut notes = Vec::new();
    if let Err(why) = crate::git::refs::fetch(cwd, REMOTE, TRACKER_REF) {
        notes.push(Note::Offline(first_line(&why)));
    }
    match standing::reassess(cwd) {
        Ok(Some(Resolution::Diverged)) => notes.push(Note::Diverged),
        Ok(Some(Resolution::ReadTracking)) => notes.push(Note::Blocked),
        // A fast-forward shows up as a moved sha below, which is the fact worth printing.
        Ok(_) => {},
        Err(why) => notes.push(Note::Offline(first_line(&why))),
    }
    // Asked of git rather than inferred from the resolution: the ref also moves when another
    // `trck` in another terminal writes to it, which is ordinary here and impossible for
    // every other verb. Either way the answer is the same question — what does it point at
    // now — so there is one place that can be wrong about it.
    let now = crate::git::rev_parse(cwd, served).ok().flatten();
    if let Some(sha) = &now
        && previous.is_some_and(|was| was != sha)
    {
        notes.push(Note::Moved(short(sha)));
    }
    (notes, now)
}

/// git's own words, trimmed to the line that says what happened. Its failures run to several
/// lines of advice about branches that are not this one.
fn first_line(why: &str) -> String {
    why.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or(why).to_string()
}

fn short(sha: &str) -> String {
    sha.chars().take(8).collect()
}

/// Refresh `ctx`'s ref forever, reporting what changes.
///
/// Returns immediately — saying why — when there is nothing a timer could discover: a
/// directory tracker has no ref, and a repository with no remote has nowhere for one to move
/// from. Both are ordinary, so both are a note rather than an error, and neither leaves a
/// thread awake to find that out again every thirty seconds.
pub(crate) fn run(ctx: &crate::discovery::Ctx, every: Duration) {
    let Source::Ref { rev, cwd } = &ctx.source else {
        return;
    };
    if !crate::git::refs::has_remote(cwd, REMOTE) {
        log(&format!("note: no `{REMOTE}` remote, so nothing can move {rev} but this machine; not polling"));
        return;
    }
    let mut said: Vec<Note> = Vec::new();
    let mut sha = crate::git::rev_parse(cwd, rev).ok().flatten();
    loop {
        let (notes, now) = tick(cwd, rev, sha.as_deref());
        // Only what is new. A state that persists is reported when it arrives, so a diverged
        // pair is one line rather than one line every interval — and coming back from
        // unreachable is worth a line of its own, since otherwise the last thing anybody
        // watching ever read was that the remote was gone.
        if said.iter().any(is_offline) && !notes.iter().any(is_offline) {
            log(&format!("note: {REMOTE} is reachable again"));
        }
        for note in notes.iter().filter(|n| !said.contains(n)) {
            log(&note.line());
        }
        (said, sha) = (notes, now);
        std::thread::sleep(every);
    }
}

fn is_offline(note: &Note) -> bool {
    matches!(note, Note::Offline(_))
}

/// The running log goes to stderr.
///
/// The startup banner is on stdout, because it is the verb's output the way every other
/// verb's is. Everything after it is this process talking about itself while it runs, which
/// is what stderr is for — and it keeps `trck serve > somewhere` a file holding the one line
/// that says where to point a browser.
fn log(line: &str) {
    eprintln!("{line}");
}

#[cfg(test)]
mod tests {
    // Tests assert; that is their job. The crate denies unwrap/expect/panic because a
    // malformed tracker must produce a diagnostic rather than a stack trace, but a test
    // that cannot panic cannot fail.
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    /// A value that is not an interval, however it fails to be one, gets the sentence that
    /// says what one is. Absent means the default, which is what makes `trck serve` a whole
    /// invocation; zero means never, which has to be sayable.
    #[test]
    fn the_interval_parses_or_says_what_an_interval_is() {
        assert_eq!(interval_from(None), Ok(Some(DEFAULT_INTERVAL)));
        assert_eq!(interval_from(Some("5")), Ok(Some(Duration::from_secs(5))));
        assert_eq!(interval_from(Some("0")), Ok(None));
        for bad in ["never", "-1", "2.5", "5s", ""] {
            let err = interval_from(Some(bad)).expect_err(bad);
            assert!(err.contains("seconds"), "{bad} was refused without saying what an interval is: {err}");
            assert!(err.contains(bad), "{bad} was refused without quoting what was typed: {err}");
        }
    }

    /// Every note has to name the branch and, where there is one, the remedy. A log line that
    /// says something is wrong without saying what to type is a line that gets ignored.
    #[test]
    fn every_note_names_the_branch_and_what_to_do() {
        for note in [Note::Offline("could not read from remote".into()), Note::Moved("abc12345".into()), Note::Diverged, Note::Blocked] {
            let line = note.line();
            assert!(line.contains(TRACKER_REF), "a note that does not name the branch: {line}");
            assert!(line.len() < 200, "a log line nobody will read to the end of: {line}");
        }
        assert!(Note::Diverged.line().contains("trck sync"), "the diverged note must name the remedy");
        assert!(Note::Blocked.line().contains("worktree"), "the blocked note must say what is holding the branch");
        assert!(Note::Offline("boom".into()).line().contains("boom"), "the offline note must carry git's own words");
    }

    /// git's failures run to several lines of advice about branches that are not this one,
    /// and a log line is one line.
    #[test]
    fn a_git_failure_is_trimmed_to_the_line_that_says_what_happened() {
        let noisy = "\nfatal: could not read from remote repository\n\nPlease make sure you have the correct access rights\nand the repository exists.\n";
        assert_eq!(first_line(noisy), "fatal: could not read from remote repository");
        assert_eq!(first_line("one line"), "one line");
        assert_eq!(first_line(""), "");
    }

    #[test]
    fn a_sha_is_shortened_for_reading_not_for_resolving() {
        assert_eq!(short("0123456789abcdef0123456789abcdef01234567"), "01234567");
        assert_eq!(short("abc"), "abc");
    }
}

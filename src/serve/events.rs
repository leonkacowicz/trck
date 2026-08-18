//! Telling open pages that the tracker moved.
//!
//! Server-sent events over a connection the server already holds, rather than the page asking
//! again every few seconds: one timer instead of two, and the timer is the one that already
//! exists — [`super::poll`]'s. A browser gets `EventSource` for free, including reconnection,
//! which is most of what a hand-rolled poll would have had to reimplement.
//!
//! **A beacon, not a subscriber list.** Every stream waits on one condition variable and
//! remembers which announcement it last saw; a change bumps a sequence number and wakes them
//! all. There is nothing to register and nothing to unregister, so a connection that dies in
//! any of the ways connections die cannot leak an entry — which is the failure mode a list of
//! senders has and this does not.
//!
//! **The payload is the version, and the version is the tracker's own.** For a ref-backed
//! tracker that is the commit the served ref points at, so the write path and the poll loop
//! cannot disagree about what "current" means. The page does not read it: it re-renders on any
//! event, because the beacon has already decided what counts as a change.
//!
//! There is one beacon per process and it is a `static` — but every operation on it is a
//! method, so a test can build its own and assert against it rather than racing every other
//! test in the binary through the shared one.

use std::io::Write;
use std::sync::{Condvar, Mutex, PoisonError};
use std::time::Duration;

/// How long a stream sits quiet before it says something anyway.
///
/// The heartbeat is the liveness check in both directions. A write to a client that has gone
/// away fails, which is how a stream thread learns to stop; and a client that hears nothing at
/// all cannot tell a quiet tracker from a dead server.
pub(crate) const HEARTBEAT: Duration = Duration::from_secs(20);

/// How long a browser should wait before reconnecting, in milliseconds. Stated rather than
/// left to the default, which is three seconds and assumes a network.
pub(crate) const RETRY_MS: u64 = 1000;

/// What every stream is waiting for.
pub(crate) struct Beacon {
    state: Mutex<Version>,
    changed: Condvar,
}

struct Version {
    /// Bumped on every announcement. What a stream compares against, so that a change landing
    /// between two waits is delivered rather than slept through.
    seq: u64,
    /// What the tracker is at now, in whatever terms it has. Empty before the first
    /// announcement, which is what a stream opening on a quiet tracker sees.
    token: String,
}

/// The one every stream and every writer in this process shares.
static BEACON: Beacon = Beacon::new();

impl Beacon {
    pub(crate) const fn new() -> Beacon {
        Beacon { state: Mutex::new(Version { seq: 0, token: String::new() }), changed: Condvar::new() }
    }

    /// Say that the tracker is now at `version`, waking every open stream.
    ///
    /// `None` is "something changed and there is no name for the new state" — a directory
    /// tracker, which has no ref to point at. It always counts as new, because with nothing to
    /// compare there is no way to tell a repeat from a change, and the wrong guess is the
    /// silent one.
    ///
    /// A repeated `Some` of the same version is not an announcement. The poll loop says this
    /// on every tick, and a page that re-rendered every thirty seconds would be the polling
    /// this exists to remove.
    pub(crate) fn announce(&self, version: Option<&str>) {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        if version.is_some_and(|v| v == state.token) {
            return;
        }
        state.seq += 1;
        // A tracker with no version of its own still needs a token that differs from the last,
        // or a page comparing them would see no change where there was one.
        state.token = version.map_or_else(|| state.seq.to_string(), str::to_string);
        drop(state);
        self.changed.notify_all();
    }

    fn now(&self) -> (u64, String) {
        let state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        (state.seq, state.token.clone())
    }

    /// Wait for an announcement after `seen`, or give up after `within`.
    fn next_after(&self, seen: u64, within: Duration) -> Option<(u64, String)> {
        let state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let (state, timed_out) = self.changed.wait_timeout_while(state, within, |s| s.seq == seen).unwrap_or_else(PoisonError::into_inner);
        (!timed_out.timed_out()).then(|| (state.seq, state.token.clone()))
    }

    /// Hold this connection open and write an event whenever the tracker moves.
    ///
    /// Returns when the client goes away, which is the only way it ends: every write is
    /// checked, and a page whose tab closed fails the next one. Nothing here reads from the
    /// socket — `EventSource` sends one request and then only listens — so a stream costs a
    /// thread and a wait, not a poll.
    ///
    /// `since` is the version the page was rendered from. If the tracker has moved past it,
    /// the first event goes out immediately: without that, a change landing between the page
    /// fetching its model and its `EventSource` connecting would wait for the *next* change,
    /// which on a quiet tracker is never.
    ///
    /// `heartbeat` is how long it sits quiet before saying something anyway. The server passes
    /// [`HEARTBEAT`]; a test passes something it is willing to wait for.
    pub(crate) fn stream(&self, out: &mut impl Write, since: Option<&str>, heartbeat: Duration) -> std::io::Result<()> {
        let mut seen = self.open(out, since)?;
        loop {
            if let Some(next) = self.next_after(seen.0, heartbeat) {
                emit(out, &next.1)?;
                seen = next;
            } else {
                // A comment: one line, it keeps the connection warm, and it is what fails when
                // the reader has gone.
                out.write_all(b": still here\n\n")?;
                out.flush()?;
            }
        }
    }

    /// Write the head, catch the page up if it is behind, and answer with what it has now seen.
    fn open(&self, out: &mut impl Write, since: Option<&str>) -> std::io::Result<(u64, String)> {
        let seen = self.now();
        write!(out, "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-store\r\nConnection: close\r\n\r\nretry: {RETRY_MS}\n\n")?;
        out.flush()?;
        if !seen.1.is_empty() && since != Some(seen.1.as_str()) {
            emit(out, &seen.1)?;
        }
        Ok(seen)
    }

    /// The sequence number, for a test asserting what an announcement did.
    #[cfg(test)]
    pub(crate) fn seq(&self) -> u64 {
        self.now().0
    }

    /// The current version, likewise.
    #[cfg(test)]
    pub(crate) fn token(&self) -> String {
        self.now().1
    }
}

/// Announce on this process's beacon.
pub(crate) fn announce(version: Option<&str>) {
    BEACON.announce(version);
}

/// Stream from this process's beacon.
pub(crate) fn stream(out: &mut impl Write, since: Option<&str>, heartbeat: Duration) -> std::io::Result<()> {
    BEACON.stream(out, since, heartbeat)
}

/// What the tracker being served is at right now.
///
/// **The served rev, not the branch a write lands on.** In a clone that has never written,
/// those are two different refs — the write goes to the local branch and the page is rendered
/// from the remote-tracking one — and if the write path announced one version while the poll
/// loop announced the other, every tick would look like movement and every open page would
/// re-render on a timer. One question, asked in one place.
///
/// `None` for a directory tracker, which has no ref to point at.
pub(crate) fn version(ctx: &crate::discovery::Ctx) -> Option<String> {
    let crate::discovery::Source::Ref { rev, cwd } = &ctx.source else {
        return None;
    };
    crate::git::rev_parse(cwd, rev).ok().flatten()
}

fn emit(out: &mut impl Write, token: &str) -> std::io::Result<()> {
    // One line, always. A token carrying a newline would end the event early and put the rest
    // in the next one, where it would read as a different version entirely. A version is a sha
    // or a small integer, so this is a guard rather than a transformation — but the guard is
    // what makes that true rather than assumed.
    let token: String = token.chars().filter(|c| *c != '\n' && *c != '\r').collect();
    write!(out, "data: {token}\n\n")?;
    out.flush()
}

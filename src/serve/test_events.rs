//! What an open page is told, and when.
//!
//! Its own file, the way `test_http.rs` is. Each test builds a [`Beacon`] of its own rather
//! than reaching for the process-global one: the write path announces on that one too, so a
//! test asserting against it would be racing every other test in the binary that happens to
//! apply an edit.

// Tests assert; that is their job. The crate denies unwrap/expect/panic because a malformed
// tracker must produce a diagnostic rather than a stack trace, but a test that cannot panic
// cannot fail.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::events::{Beacon, RETRY_MS};
use std::io::Write;
use std::time::Duration;

/// A sink that gives up once a *complete* message containing `stop` has gone out, which is how
/// a real stream ends: the client goes away, and the next write is what says so.
///
/// Complete matters. `write!` issues one call per fragment of its format string, so a sink that
/// stopped the moment it saw `data: ` would cut the event off before its own payload and the
/// test would be asserting against half a message it had truncated itself. Every message here
/// ends with a blank line, so that is the boundary.
struct Until {
    written: Vec<u8>,
    stop: &'static str,
    done: bool,
}

impl Write for Until {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if self.done {
            return Err(std::io::Error::new(std::io::ErrorKind::BrokenPipe, "gone"));
        }
        self.written.extend_from_slice(buf);
        self.done = self.written.ends_with(b"\n\n") && String::from_utf8_lossy(&self.written).contains(self.stop);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Run a stream until it has written `stop`, and answer with everything it wrote.
fn streamed(beacon: &Beacon, since: Option<&str>, stop: &'static str) -> String {
    let mut sink = Until { written: Vec::new(), stop, done: false };
    let err = beacon.stream(&mut sink, since, Duration::from_millis(10)).expect_err("a stream ends when its client does");
    assert_eq!(err.kind(), std::io::ErrorKind::BrokenPipe);
    String::from_utf8_lossy(&sink.written).into_owned()
}

/// The poll loop announces on every tick. Only a *change* may wake an open page — one that
/// re-rendered every thirty seconds would be the polling this exists to remove.
#[test]
fn an_announcement_counts_only_when_the_version_actually_changed() {
    let beacon = Beacon::new();
    beacon.announce(Some("aaaa1111"));
    let first = beacon.seq();
    assert_eq!(beacon.token(), "aaaa1111");

    beacon.announce(Some("aaaa1111"));
    assert_eq!(beacon.seq(), first, "an unchanged version woke every open page");

    beacon.announce(Some("bbbb2222"));
    assert_eq!(beacon.seq(), first + 1);
    assert_eq!(beacon.token(), "bbbb2222");

    // A tracker with no version of its own: every announcement is new, because with nothing to
    // compare there is no way to tell a repeat from a change, and the wrong guess is the
    // silent one.
    beacon.announce(None);
    let anonymous = beacon.token();
    beacon.announce(None);
    assert_ne!(beacon.token(), anonymous, "two writes to a directory tracker looked like one");
}

/// A quiet tracker still says something, because a client that hears nothing cannot tell it
/// apart from a server that has gone — and the heartbeat is also what fails, and so ends the
/// thread, when the client is the one that went.
#[test]
fn a_stream_with_nothing_to_report_still_writes_a_heartbeat() {
    let beacon = Beacon::new();
    beacon.announce(Some("cccc3333"));
    // Already current, so there is no catch-up and the first thing after the head is the
    // heartbeat.
    let text = streamed(&beacon, Some("cccc3333"), ": still here");
    assert!(text.contains("Content-Type: text/event-stream"), "{text}");
    assert!(text.contains("Cache-Control: no-store"), "{text}");
    assert!(text.contains(&format!("retry: {RETRY_MS}")), "a browser is not told how soon to come back: {text}");
    assert!(!text.contains("data: "), "a quiet tracker sent an event: {text}");
}

/// A page that connects already behind is told at once. Without this, a change landing between
/// the page fetching its model and its `EventSource` connecting would wait for the *next*
/// change — which on a quiet tracker is never.
#[test]
fn a_stream_that_opens_behind_is_caught_up_and_one_that_is_level_is_not() {
    let beacon = Beacon::new();
    beacon.announce(Some("dddd4444"));
    let text = streamed(&beacon, Some("an older version"), "data: ");
    assert!(text.contains("data: dddd4444"), "a page that opened behind was not caught up: {text}");

    let text = streamed(&beacon, Some("dddd4444"), ": still here");
    assert!(!text.contains("data: "), "a page that was already current was told to re-render: {text}");
}

/// A page with no version of its own — one served from a directory tracker — is caught up to
/// whatever there is, rather than being left to wait for a change it has no baseline for.
#[test]
fn a_page_with_no_version_is_told_where_things_stand() {
    let beacon = Beacon::new();
    beacon.announce(Some("eeee5555"));
    let text = streamed(&beacon, None, "data: ");
    assert!(text.contains("data: eeee5555"), "{text}");
}

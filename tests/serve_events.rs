//! The event stream, over a real socket.
//!
//! What the unit tests beside `src/serve/` cannot reach: a connection genuinely held open while
//! the tracker moves underneath it, from both of the causes that move it — a write through this
//! process, and a fast-forward found by the poll loop. Everything here reads the stream as it
//! arrives rather than to the end, because it has no end.
//!
//! The browser half is not here and cannot be: `EventSource` is the browser's, and what this
//! asserts is that what arrives on the wire is what one is defined to consume.

// Tests assert; that is their job. The crate denies unwrap/expect/panic because a malformed
// tracker must produce a diagnostic rather than a stack trace, but a test that cannot panic
// cannot fail.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::{Scenario, Server, TRACKER_BRANCH, clone_of, git_must, trck_must};
use std::io::{BufRead as _, BufReader, Read as _, Write as _};
use std::net::{Ipv4Addr, TcpStream};
use std::time::{Duration, Instant};

/// How long a waiting assertion gives up after — generous, because what is being waited for is
/// another process's timer and a `git fetch` over a local path.
const DEADLINE: Duration = Duration::from_secs(30);

/// A held-open `/events` connection, read line by line as the server writes them.
struct Listening {
    lines: BufReader<TcpStream>,
}

impl Listening {
    /// Connect and read past the response head, leaving the body to be read as it arrives.
    fn open(server: &Server, since: Option<&str>) -> Listening {
        let query = since.map_or_else(String::new, |v| format!("?v={v}"));
        let mut sock = TcpStream::connect((Ipv4Addr::LOCALHOST, server.port)).expect("connecting");
        sock.set_read_timeout(Some(DEADLINE)).expect("a deadline");
        write!(sock, "GET /events{query} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nAccept: text/event-stream\r\n\r\n", server.port).expect("the request");
        let mut lines = BufReader::new(sock);
        let mut head = String::new();
        loop {
            let mut line = String::new();
            assert!(lines.read_line(&mut line).expect("reading the head") > 0, "the stream closed before its head was done:\n{head}");
            head.push_str(&line);
            if line == "\r\n" {
                break;
            }
        }
        assert!(head.starts_with("HTTP/1.1 200 OK\r\n"), "{head}");
        assert!(head.contains("Content-Type: text/event-stream"), "{head}");
        Listening { lines }
    }

    /// The next `data:` payload, skipping heartbeats and blank lines. `None` on the deadline.
    fn next_event(&mut self) -> Option<String> {
        let until = Instant::now() + DEADLINE;
        while Instant::now() < until {
            let mut line = String::new();
            if self.lines.read_line(&mut line).ok()? == 0 {
                return None;
            }
            if let Some(payload) = line.strip_prefix("data: ") {
                return Some(payload.trim_end().to_string());
            }
        }
        None
    }
}

fn sha(s: &Scenario, rev: &str) -> String {
    git_must(&s.work, &["rev-parse", rev])
}

/// A server over the fixture clone with a local tracker branch, polling fast enough for a test
/// to wait on.
fn serving(s: &Scenario) -> Server {
    git_must(&s.work, &["branch", TRACKER_BRANCH, &format!("origin/{TRACKER_BRANCH}")]);
    Server::start(&s.work, &["--poll", "1"])
}

/// **The first cause.** A write through this process reaches a page that was already open,
/// carrying the version the tracker is now at.
#[test]
fn a_write_through_this_process_reaches_an_open_stream() {
    let Some(s) = Scenario::build("sse-write") else {
        return;
    };
    let server = serving(&s);
    let before = sha(&s, TRACKER_BRANCH);
    let mut listening = Listening::open(&server, Some(&before));

    let body = r#"{"edits": [{"id": "aaaaaaa", "field": "status", "value": "in-progress"}]}"#;
    let res = server.request(&format!(
        "POST /edits HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        server.port,
        body.len()
    ));
    assert!(res.contains("\"ok\": true"), "{res}");

    let event = listening.next_event().expect("an event");
    assert_eq!(event, sha(&s, TRACKER_BRANCH), "the event does not name the version the tracker is now at");
    assert_ne!(event, before, "the event named the version the page already had");
}

/// **The second cause.** Somebody else pushes, the poll loop fast-forwards, and the same stream
/// says so — without the page having asked for anything.
#[test]
fn a_fast_forward_found_by_the_poll_loop_reaches_an_open_stream() {
    let Some(s) = Scenario::build("sse-poll") else {
        return;
    };
    let server = serving(&s);
    let before = sha(&s, TRACKER_BRANCH);
    let mut listening = Listening::open(&server, Some(&before));

    let elsewhere = clone_of(s.work.parent().expect("a parent"), &s.origin, "elsewhere", &[]);
    trck_must(&elsewhere, &["new", "Landed elsewhere", "--id", "ddddddd", "--empty"]);

    let event = listening.next_event().expect("an event");
    assert_ne!(event, before, "the event named the version the page already had");
    assert_eq!(event, sha(&s, TRACKER_BRANCH), "the event does not name the fast-forwarded version");

    // And the model the page would fetch on that event holds the new issue — which is the
    // whole point, since the event carries a version and nothing else.
    let model = server.get("/model");
    assert!(model.contains("Landed elsewhere"), "the model does not hold what the event announced");
    assert!(model.contains(&event), "the model does not carry the version it was built from");
}

/// A quiet tracker is not a silent connection. Without a heartbeat a page could not tell a
/// tracker where nothing is happening from a server that has gone away.
#[test]
fn a_quiet_tracker_still_sends_a_heartbeat_and_no_events() {
    let Some(s) = Scenario::build("sse-quiet") else {
        return;
    };
    let server = serving(&s);
    let mut listening = Listening::open(&server, Some(&sha(&s, TRACKER_BRANCH)));

    // Read raw for a couple of poll ticks: the poll loop runs every second here, and a beacon
    // that announced on every tick rather than on every change would show up as events.
    let mut buf = [0u8; 1024];
    listening.lines.get_mut().set_read_timeout(Some(Duration::from_secs(3))).expect("a deadline");
    let read = listening.lines.get_mut().read(&mut buf).unwrap_or(0);
    let text = String::from_utf8_lossy(&buf[..read]);
    assert!(!text.contains("data: "), "an unchanged tracker woke the page: {text:?}");
}

/// Streams have a bound of their own, well under the connection cap, so that a drawer full of
/// forgotten tabs cannot leave nothing to serve the page they are watching with.
#[test]
fn held_open_streams_cannot_exhaust_the_server() {
    let Some(s) = Scenario::build("sse-cap") else {
        return;
    };
    let server = serving(&s);
    // More than the stream allowance, well under the connection one. The ones past the bound
    // are refused; what matters is that the server is still answering afterwards.
    let held: Vec<Listening> = (0..16).map(|_| Listening::open(&server, None)).collect();
    assert_eq!(held.len(), 16);

    let over = server.request(&format!("GET /events HTTP/1.1\r\nHost: 127.0.0.1:{}\r\n\r\n", server.port));
    assert!(over.starts_with("HTTP/1.1 503 "), "the seventeenth stream was not turned away: {over}");

    // The point of the bound: ordinary requests still work with the allowance full.
    assert!(server.get("/").starts_with("HTTP/1.1 200 OK\r\n"), "a full stream allowance stopped the page being served");
    assert!(server.get("/model").starts_with("HTTP/1.1 200 OK\r\n"), "a full stream allowance stopped the model being served");
}

/// The stream is a `GET`. A `POST` to it is a 405 like any other write to a read-only route,
/// rather than a connection held open by something that was not asking to listen.
#[test]
fn the_stream_route_takes_get_alone() {
    let Some(s) = Scenario::build("sse-method") else {
        return;
    };
    let server = serving(&s);
    let res = server.request(&format!("POST /events HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nContent-Length: 0\r\n\r\n", server.port));
    assert!(res.starts_with("HTTP/1.1 405 "), "{res}");
}

/// The model route answers the same document the page was rendered from, and says which
/// version it is — which is what a reconnecting stream is told, so the two must agree.
#[test]
fn the_model_route_answers_the_document_the_page_is_built_from() {
    let Some(s) = Scenario::build("sse-model") else {
        return;
    };
    let server = serving(&s);
    let model = server.get("/model");
    assert!(model.contains("application/json"), "{}", &model[..model.len().min(200)]);
    assert!(model.contains("Seeded issue"), "the model does not hold the tracker");
    assert!(model.contains(&sha(&s, TRACKER_BRANCH)), "the model does not say which version it is");
    // The page carries the same version, so its `EventSource` can say where it is.
    assert!(server.get("/").contains(&sha(&s, TRACKER_BRANCH)), "the page does not carry its own version");
}

//! `trck serve` against a real socket.
//!
//! The unit tests beside `src/serve/` parse request heads and route them; none of them binds
//! anything. What is only true of the process is what this file asserts: that it listens where
//! it said it would, that a second one refuses the busy port by name, and that Ctrl-C leaves
//! the port free — the last being the whole of the shutdown contract, since nothing installs a
//! signal handler and the kernel is what closes the listener.
//!
//! What the process does to the *tracker ref* while it runs is `serve_poll.rs`, which needs a
//! repository to have a ref in; everything here points at a directory tracker on purpose, so
//! nothing in this file depends on git at all.
//!
//! The requests are written by hand rather than through a client library, which is the same
//! reason the server is: it is a request line and two headers, and the bytes on the wire are
//! exactly what is under test.

// Tests assert; that is their job. The crate denies unwrap/expect/panic because a malformed
// tracker must produce a diagnostic rather than a stack trace, but a test that cannot panic
// cannot fail.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::Server;
use std::path::PathBuf;
use std::process::Command;

/// Where the binary is run from. Irrelevant to what it does — every server here is pointed at
/// its tracker with `--dir` — but it has to be somewhere that exists.
fn here() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The bundled example tracker: a directory, so this file needs no git repository of its own.
fn tracker() -> String {
    let dir = here().join("examples").join("action-game");
    assert!(dir.join("index.jsonl").is_file(), "no tracker at {}", dir.display());
    dir.display().to_string()
}

/// A server over the example tracker, with polling off: there is no ref to poll, and this
/// file is about the socket.
fn serving() -> Server {
    Server::start(&here(), &["--dir", &tracker(), "--poll", "0"])
}

/// Everything the socket is supposed to answer, in one process: a page, an asset, and each
/// refusal. One server rather than five because starting one is the slow part, and none of
/// these requests can affect another — the process holds no state between them.
#[test]
fn the_page_and_the_assets_come_back_over_the_socket() {
    let server = serving();
    assert!(server.banner.contains("127.0.0.1"), "the startup line does not say where it is listening: {:?}", server.banner);
    assert!(server.banner.contains("loopback"), "the startup line does not say the listener is loopback-only: {:?}", server.banner);

    let page = server.get("/");
    assert!(page.starts_with("HTTP/1.1 200 OK\r\n"), "{}", &page[..page.len().min(120)]);
    assert!(page.contains("Content-Type: text/html; charset=utf-8\r\n"), "the page came back as the wrong type");
    // The page `trck html` writes, from a socket: the doctype, the data island the script
    // reads, and a title from the tracker that was actually loaded.
    assert!(page.contains("<!doctype html>"), "that is not the page");
    assert!(page.contains("id=\"trck-data\""), "the page has no data island");
    assert!(page.contains("action-game"), "the page did not render the tracker it was pointed at");

    let css = server.get("/app.css");
    assert!(css.starts_with("HTTP/1.1 200 OK\r\n") && css.contains("text/css"), "{}", &css[..css.len().min(120)]);
    let js = server.get("/app.js");
    assert!(js.starts_with("HTTP/1.1 200 OK\r\n") && js.contains("text/javascript"), "{}", &js[..js.len().min(120)]);

    assert!(server.get("/nope").starts_with("HTTP/1.1 404 "), "an unknown path should be a 404");
    let post = server.request(&format!("POST / HTTP/1.1\r\nHost: 127.0.0.1:{}\r\n\r\n", server.port));
    assert!(post.starts_with("HTTP/1.1 405 "), "this verb does not write yet, so a POST is a 405: {post}");
    let rebound = server.request("GET / HTTP/1.1\r\nHost: tracker.example.com\r\n\r\n");
    assert!(rebound.starts_with("HTTP/1.1 403 "), "a request naming another host should be refused: {rebound}");
    let nonsense = server.request("nonsense\r\n\r\n");
    assert!(nonsense.starts_with("HTTP/1.1 400 "), "a malformed request line should be a 400: {nonsense}");

    // Still up after all of that — a refusal answers the request, it does not end the process.
    assert!(server.get("/").starts_with("HTTP/1.1 200 OK\r\n"), "the server did not survive its own refusals");
}

/// A directory tracker has no ref, so the timer has nothing it could discover and no thread is
/// left awake to find that out again every interval. Silence on stderr is the assertion.
#[test]
fn a_directory_tracker_is_not_polled() {
    let server = Server::start(&here(), &["--dir", &tracker()]);
    assert!(server.get("/").starts_with("HTTP/1.1 200 OK\r\n"), "the server did not come up");
    assert_eq!(server.log(), "", "a directory tracker has no ref to poll, so nothing should be said about one");
}

/// A second `serve` on the same port, which is what happens when one is already running in
/// another terminal. The diagnostic has to name the port: that is the fact the user needs and
/// the one thing the io error leaves out.
#[test]
fn a_busy_port_is_refused_with_a_diagnostic_naming_it() {
    let server = serving();
    let second = Command::new(env!("CARGO_BIN_EXE_trck"))
        .args(["serve", "--port", &server.port.to_string(), "--dir", &tracker()])
        .output()
        .expect("spawning a second trck serve");
    let err = String::from_utf8_lossy(&second.stderr);
    assert!(!second.status.success(), "a busy port must not look like a successful start");
    assert!(err.contains(&server.port.to_string()), "the diagnostic does not name the port: {err}");
    assert!(!err.contains("panicked"), "a busy port is a diagnostic, not a stack trace: {err}");
}

/// The whole of the shutdown contract. Nothing installs a signal handler — that would need a
/// raw `signal()` call and `unsafe` is forbidden in this crate — so what is asserted is the
/// outcome rather than the mechanism: after Ctrl-C, the port is bindable again.
///
/// Unix only, because `SIGINT` is: `Child::kill` sends `SIGKILL`, which would prove nothing
/// about the signal a user actually sends.
#[cfg(unix)]
#[test]
fn ctrl_c_leaves_the_port_free() {
    // Imported here rather than at the top: this is the only test that rebinds, and on a
    // platform without `SIGINT` the whole test is gone — leaving an unused import behind,
    // which `-D warnings` fails the Windows build over.
    use std::net::{Ipv4Addr, TcpListener};

    let mut server = serving();
    assert!(server.get("/").starts_with("HTTP/1.1 200 OK\r\n"), "the server was not up to begin with");
    let port = server.port;

    let signalled = Command::new("kill").args(["-INT", &server.pid().to_string()]).status().expect("sending SIGINT");
    assert!(signalled.success(), "could not signal the server");
    // A poller thread must not keep the process alive past the signal either; the default
    // disposition kills the process whatever its threads are doing, and that is the point.
    assert!(server.wait_exit(10), "the server did not exit on SIGINT");

    // The listener went with the process. Not "eventually": the socket was closed by the
    // kernel as the process died.
    let rebound = TcpListener::bind((Ipv4Addr::LOCALHOST, port));
    assert!(rebound.is_ok(), "port {port} is still bound after SIGINT: {rebound:?}");
}

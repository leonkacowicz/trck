//! `trck serve` against a real socket.
//!
//! The unit tests beside `src/serve/` parse request heads and route them; none of them binds
//! anything. What is only true of the process is what this file asserts: that it listens where
//! it said it would, that a second one refuses the busy port by name, and that Ctrl-C leaves
//! the port free — the last being the whole of the shutdown contract, since nothing installs a
//! signal handler and the kernel is what closes the listener.
//!
//! The requests are written by hand rather than through a client library, which is the same
//! reason the server is: it is a request line and two headers, and the bytes on the wire are
//! exactly what is under test.

// Tests assert; that is their job. The crate denies unwrap/expect/panic because a malformed
// tracker must produce a diagnostic rather than a stack trace, but a test that cannot panic
// cannot fail.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::io::{BufRead as _, BufReader, Read as _, Write as _};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

/// The bundled example tracker: a directory, so this test needs no git repository of its own.
fn tracker() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples").join("action-game");
    assert!(dir.join("index.jsonl").is_file(), "no tracker at {}", dir.display());
    dir
}

/// A `trck serve` on an ephemeral port, killed however the test ends.
struct Server {
    child: Child,
    port: u16,
    banner: String,
}

impl Server {
    /// Start on `--port 0` and read back the port the OS chose.
    ///
    /// The startup line is the handshake as well as the announcement: it is written after the
    /// bind, so a test that has read it has a listener to talk to and needs no sleep.
    fn start() -> Server {
        let mut child = Command::new(env!("CARGO_BIN_EXE_trck"))
            .args(["serve", "--port", "0", "--dir"])
            .arg(tracker())
            .env("NO_COLOR", "1")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawning trck serve");
        let mut out = BufReader::new(child.stdout.take().expect("stdout is piped"));
        let mut banner = String::new();
        out.read_line(&mut banner).expect("the startup line");
        let port = banner
            .rsplit_once(':')
            .map(|(_, tail)| tail.trim_start_matches(|c: char| !c.is_ascii_digit()))
            .and_then(|tail| tail.split(|c: char| !c.is_ascii_digit()).next())
            .and_then(|digits| digits.parse().ok())
            .unwrap_or_else(|| panic!("no port in the startup line: {banner:?}"));
        Server { child, port, banner }
    }

    /// Send a request verbatim and read the whole response. The server closes the connection
    /// after answering, which is what ends the read.
    fn request(&self, raw: &str) -> String {
        let mut sock = TcpStream::connect((Ipv4Addr::LOCALHOST, self.port)).expect("connecting");
        sock.write_all(raw.as_bytes()).expect("writing the request");
        let mut answer = Vec::new();
        sock.read_to_end(&mut answer).expect("reading the response");
        String::from_utf8_lossy(&answer).into_owned()
    }

    fn get(&self, path: &str) -> String {
        self.request(&format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nConnection: close\r\n\r\n", self.port))
    }
}

impl Drop for Server {
    /// A failing assertion unwinds past any teardown written at the end of a test body, and a
    /// server left listening would fail the next run rather than this one.
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Everything the socket is supposed to answer, in one process: a page, an asset, and each
/// refusal. One server rather than five because starting one is the slow part, and none of
/// these requests can affect another — the process holds no state between them.
#[test]
fn the_page_and_the_assets_come_back_over_the_socket() {
    let server = Server::start();
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

/// A second `serve` on the same port, which is what happens when one is already running in
/// another terminal. The diagnostic has to name the port: that is the fact the user needs and
/// the one thing the io error leaves out.
#[test]
fn a_busy_port_is_refused_with_a_diagnostic_naming_it() {
    let server = Server::start();
    let second = Command::new(env!("CARGO_BIN_EXE_trck"))
        .args(["serve", "--port", &server.port.to_string(), "--dir"])
        .arg(tracker())
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
    let mut server = Server::start();
    assert!(server.get("/").starts_with("HTTP/1.1 200 OK\r\n"), "the server was not up to begin with");
    let port = server.port;

    let signalled = Command::new("kill").args(["-INT", &server.child.id().to_string()]).status().expect("sending SIGINT");
    assert!(signalled.success(), "could not signal the server");
    server.child.wait().expect("the server exits on SIGINT");

    // The listener went with the process. Not "eventually": the socket was closed by the
    // kernel as the process died, so this holds on the first attempt.
    let rebound = TcpListener::bind((Ipv4Addr::LOCALHOST, port));
    assert!(rebound.is_ok(), "port {port} is still bound after SIGINT: {rebound:?}");
}

//! What `serve`'s request parser accepts, and what it answers instead.
//!
//! Its own file, the way `test_graph.rs` and `test_index.rs` are: `http.rs` is the wire format
//! and this is the table of what arrives on the wire, and the two are read for different
//! reasons. Everything here goes through the module's own entry points — `read_head` and the
//! `Response` constructors — so the file states the behaviour rather than the internals.

// Tests assert; that is their job. The crate denies unwrap/expect/panic because a
// malformed tracker must produce a diagnostic rather than a stack trace, but a test
// that cannot panic cannot fail.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::http::{Request, Response, read_head};

fn head(raw: &str) -> Result<Option<Request>, Response> {
    read_head(raw.as_bytes())
}

#[test]
fn an_ordinary_browser_request_parses() {
    let req = head("GET /app.css?v=2 HTTP/1.1\r\nHost: localhost:8725\r\nAccept: */*\r\n\r\n").ok().flatten().expect("a request");
    assert_eq!(req.method, "GET");
    // The query is dropped: every route is a fixed path and `/?x=1` is the same document.
    assert_eq!(req.path, "/app.css");
    assert_eq!(req.host.as_deref(), Some("localhost:8725"));
}

/// `printf 'GET / HTTP/1.0\n\n' | nc` is how a person checks a server by hand, and it is
/// the one client with no library between it and the socket.
#[test]
fn bare_newlines_and_a_missing_host_are_accepted() {
    let req = head("GET / HTTP/1.0\n\n").ok().flatten().expect("a request");
    assert_eq!(req.path, "/");
    assert_eq!(req.host, None);
}

#[test]
fn the_host_header_is_matched_case_insensitively() {
    let req = head("GET / HTTP/1.1\r\nHOST: 127.0.0.1:8725\r\n\r\n").ok().flatten().expect("a request");
    assert_eq!(req.host.as_deref(), Some("127.0.0.1:8725"));
}

/// A connection that says nothing is a preconnect or a port check, not a bad request.
#[test]
fn a_silent_connection_is_answered_with_nothing() {
    assert!(head("").ok().expect("not a refusal").is_none());
}

#[test]
fn a_head_with_no_blank_line_is_refused() {
    let err = head("GET / HTTP/1.1\r\nHost: localhost\r\n").err().expect("refused");
    assert_eq!(err.code(), 400);
}

#[test]
fn a_head_past_the_cap_is_refused_rather_than_buffered() {
    // The cap is the refusal: no allocation grows with what the client sends.
    let flood = format!("GET / HTTP/1.1\r\nHost: localhost\r\nX-Big: {}\r\n\r\n", "a".repeat(16 * 1024));
    assert_eq!(head(&flood).err().expect("refused").code(), 400);
}

#[test]
fn a_malformed_request_line_is_refused() {
    for raw in ["nonsense\r\n\r\n", "GET\r\n\r\n", "GET / HTTP/2\r\n\r\n", "GET / HTTP/1.1 extra\r\n\r\n", "\r\n\r\n"] {
        assert_eq!(head(raw).err().map(|r| r.code()), Some(400), "{raw:?} should be refused");
    }
}

/// Absolute-form is what a client talking to a proxy sends, and this is not one.
#[test]
fn a_non_origin_form_target_is_refused() {
    assert_eq!(head("GET http://example.com/ HTTP/1.1\r\n\r\n").err().map(|r| r.code()), Some(400));
    assert_eq!(head("OPTIONS * HTTP/1.1\r\n\r\n").err().map(|r| r.code()), Some(400));
}

/// A non-GET still parses — refusing the *method* is routing's answer, and it needs the
/// method's name to say which one it turned down.
#[test]
fn a_non_get_method_parses_so_routing_can_name_it() {
    let req = head("POST / HTTP/1.1\r\nHost: localhost\r\n\r\n").ok().flatten().expect("a request");
    assert_eq!(req.method, "POST");
}

#[test]
fn a_response_states_its_own_body_length_and_closes() {
    let mut out: Vec<u8> = Vec::new();
    Response::html("<p>hi</p>".to_string()).write_to(&mut out).expect("written");
    let text = String::from_utf8(out).expect("utf-8");
    assert!(text.starts_with("HTTP/1.1 200 OK\r\n"), "{text}");
    assert!(text.contains("Content-Length: 9\r\n"), "{text}");
    assert!(text.contains("Content-Type: text/html; charset=utf-8\r\n"), "{text}");
    assert!(text.contains("Connection: close\r\n"), "{text}");
    // The page is rendered from the tracker as it is now; a cached copy is a tracker
    // from the past, which is the thing this verb exists to remove.
    assert!(text.contains("Cache-Control: no-store\r\n"), "{text}");
    assert!(text.ends_with("\r\n\r\n<p>hi</p>"), "{text}");
}

/// The length is in bytes, not characters — a multi-byte body counted in `chars` would
/// leave the client waiting for bytes that never come.
#[test]
fn the_content_length_counts_bytes_not_characters() {
    let mut out: Vec<u8> = Vec::new();
    Response::html("é".to_string()).write_to(&mut out).expect("written");
    assert!(String::from_utf8_lossy(&out).contains("Content-Length: 2\r\n"));
}

/// A 405 without `Allow` is a 405 the specification forbids, and the one thing the
/// client can act on is which method it should have used.
#[test]
fn a_405_names_the_method_and_carries_allow() {
    let mut out: Vec<u8> = Vec::new();
    Response::method_not_allowed("DELETE").write_to(&mut out).expect("written");
    let text = String::from_utf8(out).expect("utf-8");
    assert!(text.starts_with("HTTP/1.1 405 Method Not Allowed\r\n"), "{text}");
    assert!(text.contains("Allow: GET\r\n"), "{text}");
    assert!(text.contains("DELETE"), "{text}");
}

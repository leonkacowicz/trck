//! What each path answers with.
//!
//! Its own file, the way `test_http.rs` is. Everything here goes through `respond`, so the file
//! is the table of what this server offers rather than a tour of how it decides.

// Tests assert; that is their job. The crate denies unwrap/expect/panic because a
// malformed tracker must produce a diagnostic rather than a stack trace, but a test
// that cannot panic cannot fail.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::http::{Request, Response};
use super::route::*;
use crate::discovery::tests::Tmp;
use crate::discovery::{Ctx, Source};
use crate::graph::Graph;

/// The response a request answers with. Every test here is about a document; the one route
/// that streams instead is asserted separately, and by name.
fn answered(ctx: &Ctx, req: &Request) -> Response {
    match respond(ctx, req) {
        Answer::Now(response) => response,
        Answer::Stream(_) => panic!("{} {} was expected to answer, not stream", req.method, req.path),
    }
}

fn request(method: &str, path: &str, host: Option<&str>) -> Request {
    Request { method: method.to_string(), path: path.to_string(), query: String::new(), host: host.map(str::to_string), body: String::new() }
}

/// A tracker with one issue in it, on disk. The routes do not care which source the
/// context resolved from — `render_html` reads through `Ctx` either way — and a directory
/// is the one a test can build without a git repository.
fn tracker(tag: &str) -> (Tmp, Ctx) {
    let tmp = Tmp::new(tag);
    let dir = tmp.tracker("issues");
    std::fs::create_dir_all(dir.join(crate::discovery::ITEMS_DIR)).expect("mkdir");
    std::fs::write(dir.join(crate::discovery::ITEMS_DIR).join("aaa1111-alpha.md"), "# Alpha\n").expect("body");
    std::fs::write(
        dir.join("index.jsonl"),
        "{\"id\": \"aaa1111\", \"slug\": \"alpha\", \"title\": \"Alpha\", \"status\": \"backlog\", \"priority\": \"medium\"}\n",
    )
    .expect("index");
    let ctx = Ctx::load(Source::Dir(dir), false).expect("loads");
    (tmp, ctx)
}

fn body_of(response: &Response) -> String {
    let mut out: Vec<u8> = Vec::new();
    response.write_to(&mut out).expect("written");
    String::from_utf8(out).expect("utf-8")
}

#[test]
fn the_root_serves_the_page_the_html_verb_writes() {
    let (_tmp, ctx) = tracker("serveroot");
    let response = answered(&ctx, &request("GET", "/", Some("127.0.0.1:8725")));
    assert_eq!(response.code(), 200);
    let served = body_of(&response);
    assert!(served.contains("Content-Type: text/html; charset=utf-8"), "{}", &served[..served.len().min(200)]);
    // Not "looks like HTML" — the same bytes `render_html` produces, so a page served and
    // a page written can never be two renderings of one tracker.
    let version = crate::serve::events::version(&ctx);
    let page = crate::html::Page { cmd: None, live: true, version: version.as_deref() };
    let expected = crate::html::render_html(&ctx, &Graph::new(crate::verbs::load_rows(&ctx).expect("rows")), &page);
    assert!(served.ends_with(&expected), "the served page is not what render_html produced");
    assert!(expected.contains("Alpha"), "the fixture tracker did not render");
}

/// The page inlines them, so nothing fetches these — but if anything does, it gets the
/// bytes compiled into this binary and never a file from a working tree.
#[test]
fn the_assets_come_from_the_compiled_in_copies() {
    let (_tmp, ctx) = tracker("serveassets");
    for (path, kind, asset) in [(CSS_PATH, "text/css", crate::html::CSS), (JS_PATH, "text/javascript", crate::html::APP_JS)] {
        let response = answered(&ctx, &request("GET", path, None));
        assert_eq!(response.code(), 200, "{path}");
        let served = body_of(&response);
        assert!(served.contains(&format!("Content-Type: {kind}; charset=utf-8")), "{path} served as the wrong type");
        assert!(served.ends_with(asset), "{path} did not serve the compiled-in copy");
    }
}

#[test]
fn an_unknown_path_is_a_404_that_names_what_is_served() {
    let (_tmp, ctx) = tracker("serve404");
    let response = answered(&ctx, &request("GET", "/favicon.ico", None));
    assert_eq!(response.code(), 404);
    assert!(body_of(&response).contains("/app.css"), "the 404 does not say what is served");
}

/// This child serves; it does not write. Writes arrive with #mcmfmca, and until they do a
/// POST that was silently answered 404 would read as a route that has not landed yet.
#[test]
fn a_write_method_is_refused_as_a_method_rather_than_a_missing_route() {
    let (_tmp, ctx) = tracker("serve405");
    for method in ["POST", "PUT", "DELETE", "HEAD"] {
        let response = answered(&ctx, &request(method, "/", None));
        assert_eq!(response.code(), 405, "{method}");
    }
}

/// Binding loopback stops another *machine*; it does nothing about another *site* whose
/// hostname resolves to 127.0.0.1 and whose page can have the visitor's own browser read
/// the tracker out of this one. The `Host` the client addressed is what separates them.
#[test]
fn a_request_naming_another_host_is_refused() {
    let (_tmp, ctx) = tracker("servehost");
    let response = answered(&ctx, &request("GET", "/", Some("tracker.example.com")));
    assert_eq!(response.code(), 403);
    // The method check must not run first: a POST from a rebound host is still the
    // rebinding, and answering 405 would tell the attacker the origin is reachable.
    assert_eq!(answered(&ctx, &request("POST", "/", Some("evil.example"))).code(), 403);
}

/// The accepted list is what a browser actually sends when someone opens the URL this verb
/// prints, or types `localhost` instead. The refused list is the shape of the attack: a
/// name that merely *contains* one of these is a different host, and a substring check
/// would wave it through.
#[test]
fn every_way_of_naming_this_machine_is_accepted() {
    for host in ["127.0.0.1", "127.0.0.1:8725", "localhost", "localhost:1", "[::1]", "[::1]:8725"] {
        assert!(is_loopback_host(host), "{host} should be accepted");
    }
    // A bare, unbracketed IPv6 literal is not a legal `Host` — the brackets are what make
    // the port separator findable — so it is refused with everything else that is not one
    // of the forms above.
    for host in ["example.com", "127.0.0.1.example.com", "localhost.example.com", "192.168.1.4:8725", "::1", ""] {
        assert!(!is_loopback_host(host), "{host} should be refused");
    }
}

/// A tracker that will not parse is a diagnostic, not a dead process: the tab reloads
/// once the index is fixed.
#[test]
fn a_broken_tracker_is_a_500_carrying_the_diagnostic() {
    let tmp = Tmp::new("servebroken");
    let dir = tmp.tracker("issues");
    std::fs::write(dir.join("index.jsonl"), "not json at all\n").expect("index");
    let ctx = Ctx::load(Source::Dir(dir), false).expect("loads");
    let response = answered(&ctx, &request("GET", "/", None));
    assert_eq!(response.code(), 500);
    assert!(body_of(&response).contains("index.jsonl"), "the 500 does not name what failed");
}

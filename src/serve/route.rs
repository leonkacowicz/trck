//! What each path answers with.
//!
//! Three routes and every refusal. The page comes from [`crate::html::render_html`] — the
//! same function `trck html` writes to a file — so a served page and a written one cannot
//! disagree about what the tracker says; this file adds a socket, not a second renderer.
//!
//! The tracker is read per request rather than held. A long-lived process is the first thing
//! in this engine that can be *given* a stale tracker: another `trck` in another terminal
//! moves the local ref while this one is running, which is normal here and impossible for
//! every other verb. Re-reading costs what `trck html` costs and can never be wrong;
//! caching it on the ref SHA is #eemua4s's job, once there is a ref watcher to invalidate on.

use super::http::{Request, Response};
use crate::discovery::Ctx;
use crate::graph::Graph;

/// The paths the page's own assets sit at.
///
/// The self-contained page inlines both — that is what makes `trck html`'s output a file you
/// can mail — so a browser on `/` never fetches these. They are served anyway, from the
/// compiled-in copies rather than from any file on disk, because they are the two URLs a
/// reader will try and because nothing in this process is allowed to answer an asset request
/// out of a working tree that may be on another branch entirely.
const CSS_PATH: &str = "/app.css";
const JS_PATH: &str = "/app.js";

/// Whether `Host` names this machine's own loopback interface.
///
/// The listener is bound to 127.0.0.1, which stops another machine reaching it and does
/// nothing about another *site*: a page on the open web whose hostname resolves to 127.0.0.1
/// can have the visitor's own browser fetch this one and read the tracker out of the
/// response. Checking the host the client believed it was addressing is the standard answer,
/// and it costs one header the parser already has in hand. An absent `Host` is allowed —
/// HTTP/1.0 has none, and a rebinding attempt always carries the attacker's own domain.
fn is_loopback_host(host: &str) -> bool {
    // An IPv6 literal is bracketed, so the port separator cannot be found by splitting on
    // the last colon. Nothing can reach an IPv6 address on this listener; the form is
    // handled so the rule reads as a rule about hosts rather than about IPv4.
    let name = match host.strip_prefix('[') {
        Some(rest) => rest.split(']').next().unwrap_or(rest),
        None => host.split(':').next().unwrap_or(host),
    };
    matches!(name, "127.0.0.1" | "localhost" | "::1")
}

/// The tracker, rendered as the page `trck html` emits.
///
/// A tracker that will not load answers 500 with the diagnostic as the body. That is the
/// same text the CLI would print, and it is the only useful thing to say: the process stays
/// up, so fixing the index and reloading the tab is the whole recovery.
fn page(ctx: &Ctx) -> Response {
    match crate::verbs::load_rows(ctx) {
        Ok(rows) => Response::html(crate::html::render_html(ctx, &Graph::new(rows), None)),
        Err(e) => Response::problem(500, "Internal Server Error", &e),
    }
}

/// Answer one request.
pub(crate) fn respond(ctx: &Ctx, req: &Request) -> Response {
    if let Some(host) = &req.host
        && !is_loopback_host(host)
    {
        return Response::problem(403, "Forbidden", "this server answers only to a loopback Host; a request naming another host is not from this machine");
    }
    if req.method != "GET" {
        return Response::method_not_allowed(&req.method);
    }
    match req.path.as_str() {
        "/" => page(ctx),
        CSS_PATH => Response::asset("text/css; charset=utf-8", crate::html::CSS),
        JS_PATH => Response::asset("text/javascript; charset=utf-8", crate::html::APP_JS),
        other => Response::problem(404, "Not Found", &format!("no route for {other}; this server serves /, {CSS_PATH} and {JS_PATH}")),
    }
}

#[cfg(test)]
mod tests {
    // Tests assert; that is their job. The crate denies unwrap/expect/panic because a
    // malformed tracker must produce a diagnostic rather than a stack trace, but a test
    // that cannot panic cannot fail.
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;
    use crate::discovery::Source;
    use crate::discovery::tests::Tmp;

    fn request(method: &str, path: &str, host: Option<&str>) -> Request {
        Request { method: method.to_string(), path: path.to_string(), host: host.map(str::to_string) }
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
        let response = respond(&ctx, &request("GET", "/", Some("127.0.0.1:8725")));
        assert_eq!(response.code(), 200);
        let served = body_of(&response);
        assert!(served.contains("Content-Type: text/html; charset=utf-8"), "{}", &served[..served.len().min(200)]);
        // Not "looks like HTML" — the same bytes `render_html` produces, so a page served and
        // a page written can never be two renderings of one tracker.
        let expected = crate::html::render_html(&ctx, &Graph::new(crate::verbs::load_rows(&ctx).expect("rows")), None);
        assert!(served.ends_with(&expected), "the served page is not what render_html produced");
        assert!(expected.contains("Alpha"), "the fixture tracker did not render");
    }

    /// The page inlines them, so nothing fetches these — but if anything does, it gets the
    /// bytes compiled into this binary and never a file from a working tree.
    #[test]
    fn the_assets_come_from_the_compiled_in_copies() {
        let (_tmp, ctx) = tracker("serveassets");
        for (path, kind, asset) in [(CSS_PATH, "text/css", crate::html::CSS), (JS_PATH, "text/javascript", crate::html::APP_JS)] {
            let response = respond(&ctx, &request("GET", path, None));
            assert_eq!(response.code(), 200, "{path}");
            let served = body_of(&response);
            assert!(served.contains(&format!("Content-Type: {kind}; charset=utf-8")), "{path} served as the wrong type");
            assert!(served.ends_with(asset), "{path} did not serve the compiled-in copy");
        }
    }

    #[test]
    fn an_unknown_path_is_a_404_that_names_what_is_served() {
        let (_tmp, ctx) = tracker("serve404");
        let response = respond(&ctx, &request("GET", "/favicon.ico", None));
        assert_eq!(response.code(), 404);
        assert!(body_of(&response).contains("/app.css"), "the 404 does not say what is served");
    }

    /// This child serves; it does not write. Writes arrive with #mcmfmca, and until they do a
    /// POST that was silently answered 404 would read as a route that has not landed yet.
    #[test]
    fn a_write_method_is_refused_as_a_method_rather_than_a_missing_route() {
        let (_tmp, ctx) = tracker("serve405");
        for method in ["POST", "PUT", "DELETE", "HEAD"] {
            let response = respond(&ctx, &request(method, "/", None));
            assert_eq!(response.code(), 405, "{method}");
        }
    }

    /// Binding loopback stops another *machine*; it does nothing about another *site* whose
    /// hostname resolves to 127.0.0.1 and whose page can have the visitor's own browser read
    /// the tracker out of this one. The `Host` the client addressed is what separates them.
    #[test]
    fn a_request_naming_another_host_is_refused() {
        let (_tmp, ctx) = tracker("servehost");
        let response = respond(&ctx, &request("GET", "/", Some("tracker.example.com")));
        assert_eq!(response.code(), 403);
        // The method check must not run first: a POST from a rebound host is still the
        // rebinding, and answering 405 would tell the attacker the origin is reachable.
        assert_eq!(respond(&ctx, &request("POST", "/", Some("evil.example"))).code(), 403);
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
        let response = respond(&ctx, &request("GET", "/", None));
        assert_eq!(response.code(), 500);
        assert!(body_of(&response).contains("index.jsonl"), "the 500 does not name what failed");
    }
}

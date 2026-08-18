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
pub(super) const CSS_PATH: &str = "/app.css";
pub(super) const JS_PATH: &str = "/app.js";

/// The model the page is built from, on its own.
///
/// A re-render fetches this rather than the whole page: the document, the stylesheet and the
/// script have not changed and the browser would have to re-parse all three to learn one
/// status moved. It is the same `build_model` the page is rendered from, so the two cannot
/// come apart.
pub(crate) const MODEL_PATH: &str = "/model";

/// Where a page listens for the tracker moving.
pub(crate) const EVENTS_PATH: &str = "/events";

/// Whether `Host` names this machine's own loopback interface.
///
/// The listener is bound to 127.0.0.1, which stops another machine reaching it and does
/// nothing about another *site*: a page on the open web whose hostname resolves to 127.0.0.1
/// can have the visitor's own browser fetch this one and read the tracker out of the
/// response. Checking the host the client believed it was addressing is the standard answer,
/// and it costs one header the parser already has in hand. An absent `Host` is allowed —
/// HTTP/1.0 has none, and a rebinding attempt always carries the attacker's own domain.
pub(super) fn is_loopback_host(host: &str) -> bool {
    // An IPv6 literal is bracketed, so the port separator cannot be found by splitting on
    // the last colon. Nothing can reach an IPv6 address on this listener; the form is
    // handled so the rule reads as a rule about hosts rather than about IPv4.
    let name = match host.strip_prefix('[') {
        Some(rest) => rest.split(']').next().unwrap_or(rest),
        None => host.split(':').next().unwrap_or(host),
    };
    matches!(name, "127.0.0.1" | "localhost" | "::1")
}

/// This rendering: live, at whatever the tracker is now.
///
/// The version is read per request rather than once, because it is the thing that moves — a
/// page carrying the version of whenever the process started would tell its event stream it
/// was current when it was not.
fn live_page(version: Option<&str>) -> crate::html::Page<'_> {
    crate::html::Page { cmd: None, live: true, version }
}

/// The tracker, rendered as the page `trck html` emits.
///
/// A tracker that will not load answers 500 with the diagnostic as the body. That is the
/// same text the CLI would print, and it is the only useful thing to say: the process stays
/// up, so fixing the index and reloading the tab is the whole recovery.
fn page(ctx: &Ctx) -> Response {
    let version = super::events::version(ctx);
    match crate::verbs::load_rows(ctx) {
        Ok(rows) => Response::html(crate::html::render_html(ctx, &Graph::new(rows), &live_page(version.as_deref()))),
        Err(e) => Response::problem(500, "Internal Server Error", &e),
    }
}

/// Where the page posts what it has staged. The one route that writes.
pub(crate) const EDITS_PATH: &str = "/edits";

/// Apply a staged batch and answer with what happened.
///
/// Three outcomes, three statuses, and the document says the same thing as the status line —
/// `fetch` does not reject on a 4xx, so a page reading only the body would be right and one
/// reading only the status would be right too, and they must not be able to disagree.
///
/// A refusal from the tracker is a **422**, not a 400: the request was understood perfectly
/// and the tracker declined it, which is a fact about the tracker and is why the body carries
/// the engine's own words rather than a rewording of them.
fn edits(ctx: &Ctx, body: &str) -> Response {
    match super::apply::batch(ctx, body) {
        Err(malformed) => Response::problem(400, "Bad Request", &malformed),
        Ok(outcome) if outcome.ok() => Response::json(200, "OK", outcome.json()),
        Ok(outcome) => Response::json(422, "Unprocessable Content", outcome.json()),
    }
}

/// The model as JSON, for a page re-rendering in place.
fn model(ctx: &Ctx) -> Response {
    let version = super::events::version(ctx);
    match crate::verbs::load_rows(ctx) {
        Ok(rows) => Response::json(200, "OK", crate::html::build_model(ctx, &Graph::new(rows), &live_page(version.as_deref())).to_json()),
        Err(e) => Response::problem(500, "Internal Server Error", &e),
    }
}

/// What to do with a request: answer it, or stop answering and start streaming.
///
/// The second is not a `Response`. Every other route has its bytes in hand before the head is
/// written; an event stream has none of its bytes and will not have them for as long as the
/// page stays open, so it is the connection itself that is handed over rather than a document.
pub(crate) enum Answer {
    Now(Response),
    /// Hold this connection open, catching the page up from the version it names.
    Stream(Option<String>),
}

/// Answer one request.
pub(crate) fn respond(ctx: &Ctx, req: &Request) -> Answer {
    Answer::Now(match settle(ctx, req) {
        Ok(response) => response,
        Err(stream) => return stream,
    })
}

/// A request naming a host that is not this machine's own, refused.
///
/// Its own function so that the one guard which is a *security* rule rather than a routing one
/// reads as a single named check at the top of the list.
fn rebound(req: &Request) -> Option<Response> {
    let host = req.host.as_ref()?;
    (!is_loopback_host(host))
        .then(|| Response::problem(403, "Forbidden", "this server answers only to a loopback Host; a request naming another host is not from this machine"))
}

/// The write route, which is the only one that takes a method other than `GET`.
fn write_route(ctx: &Ctx, req: &Request) -> Response {
    if req.method == "POST" { edits(ctx, &req.body) } else { Response::method_not_allowed(&req.method, Response::ALLOW_POST) }
}

/// The routes that answer with a document. `Err` is the one that does not.
fn settle(ctx: &Ctx, req: &Request) -> Result<Response, Answer> {
    if let Some(refusal) = rebound(req) {
        return Ok(refusal);
    }
    // The write route is checked before the method guard, so that a POST to it is served and a
    // POST anywhere else is still refused as a method rather than as a missing page.
    if req.path == EDITS_PATH {
        return Ok(write_route(ctx, req));
    }
    if req.method != "GET" {
        return Ok(Response::method_not_allowed(&req.method, Response::ALLOW_GET));
    }
    // The one route that does not answer. Below the method guard, so `POST /events` is a 405
    // like every other write to a read-only route rather than a connection held open forever.
    if req.path == EVENTS_PATH {
        return Err(Answer::Stream(req.query_value("v")));
    }
    Ok(match req.path.as_str() {
        "/" => page(ctx),
        MODEL_PATH => model(ctx),
        CSS_PATH => Response::asset("text/css; charset=utf-8", crate::html::CSS),
        JS_PATH => Response::asset("text/javascript; charset=utf-8", crate::html::APP_JS),
        other => Response::problem(
            404,
            "Not Found",
            &format!("no route for {other}; this server serves /, {MODEL_PATH}, {EVENTS_PATH}, {CSS_PATH}, {JS_PATH} and {EDITS_PATH}"),
        ),
    })
}

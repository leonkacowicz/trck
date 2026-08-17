//! The slice of HTTP/1.1 a loopback page server needs, and no more.
//!
//! **Why this is hand-written.** The dependency rule permits a crate — anything that links
//! statically in qualifies — so this was decided on what it costs rather than on a
//! prohibition. What `serve` needs is one method, three fixed paths, no query parsing, no
//! chunked bodies, no TLS, no keep-alive and no content negotiation; every response body is
//! already in memory before the headers are written. That is a few dozen lines here against
//! a tree of crates to audit and build on every platform the release matrix cross-compiles
//! for. The parser is also *deliberately* narrow: it accepts what a browser on loopback
//! sends and refuses the rest, which is a smaller attack surface than a general server, not
//! a larger one.
//!
//! Every refusal is a [`Response`], not an error type — a malformed request is something to
//! answer, and making the failure path produce the answer directly is what keeps the caller
//! from having to translate between two vocabularies.

use std::io::{BufRead, BufReader, Read, Write};

/// The whole request head, capped. A client that sends more than this before the blank line
/// is not a browser addressing this page, so the cap doubles as the refusal.
const MAX_HEAD: u64 = 8 * 1024;

/// What routing depends on: the method, the path with any query stripped, and the `Host` the
/// client thought it was talking to.
pub(crate) struct Request {
    pub(crate) method: String,
    pub(crate) path: String,
    pub(crate) host: Option<String>,
}

/// One response, held whole: the body is a `String` because every route already has its
/// bytes — a rendered page or a compiled-in asset — so there is nothing to stream.
pub(crate) struct Response {
    code: u16,
    reason: &'static str,
    content_type: &'static str,
    /// Verbatim extra header lines, each `\r\n`-terminated. Only 405 uses it, for the
    /// `Allow` a 405 is required to carry; a header map for one caller would be furniture.
    extra: &'static str,
    body: String,
}

impl Response {
    /// The rendered page.
    pub(crate) fn html(body: String) -> Response {
        Response { code: 200, reason: "OK", content_type: "text/html; charset=utf-8", extra: "", body }
    }

    /// A compiled-in asset, served under the type the browser needs to honour it.
    pub(crate) fn asset(content_type: &'static str, body: &str) -> Response {
        Response { code: 200, reason: "OK", content_type, extra: "", body: body.to_string() }
    }

    /// A refusal, with the diagnostic as the body: `curl` shows it, and a browser tab that
    /// went to the wrong path says why rather than rendering nothing.
    pub(crate) fn problem(code: u16, reason: &'static str, detail: &str) -> Response {
        Response { code, reason, content_type: "text/plain; charset=utf-8", extra: "", body: format!("{code} {reason}: {detail}\n") }
    }

    /// A 405, carrying the `Allow` the status is required to come with.
    pub(crate) fn method_not_allowed(method: &str) -> Response {
        Response {
            extra: "Allow: GET\r\n",
            ..Response::problem(405, "Method Not Allowed", &format!("{method} is not served here; this page is read-only over GET"))
        }
    }

    pub(crate) fn code(&self) -> u16 {
        self.code
    }

    /// The head, byte-counted from the body rather than from anything the caller passes:
    /// a `Content-Length` that disagrees with the body is how a response desynchronises a
    /// connection, and there is no way to get it wrong if it is never stated twice.
    ///
    /// `Connection: close` because there is no keep-alive here — one request per connection,
    /// so nothing has to frame a second one. `no-store` because the page is rendered from the
    /// tracker at the moment of the request; a cached copy is a tracker from the past, which
    /// is the bug this verb exists to remove.
    fn head(&self) -> String {
        format!(
            "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\n{}\r\n",
            self.code,
            self.reason,
            self.content_type,
            self.body.len(),
            self.extra
        )
    }

    /// Write head and body. A client that hangs up mid-response is the caller's to ignore —
    /// which is why this reports the io error rather than deciding what it means.
    pub(crate) fn write_to(&self, out: &mut impl Write) -> std::io::Result<()> {
        out.write_all(self.head().as_bytes())?;
        out.write_all(self.body.as_bytes())?;
        out.flush()
    }
}

/// One `\r\n`-terminated line with its terminator trimmed, or `None` at the end of input.
///
/// A bare `\n` is accepted as well as `\r\n`: `printf 'GET / HTTP/1.0\n\n' | nc` is how a
/// person checks a server by hand, and refusing it would fail the one client with no library
/// between it and the socket.
fn next_line(src: &mut impl BufRead) -> std::io::Result<Option<String>> {
    let mut line = String::new();
    if src.read_line(&mut line)? == 0 {
        return Ok(None);
    }
    Ok(Some(line.trim_end_matches('\n').trim_end_matches('\r').to_string()))
}

/// `METHOD SP TARGET SP HTTP/x.y` — the only request line shape this serves.
///
/// The target must be origin-form (`/path`). Absolute-form is what a client talking to a
/// proxy sends, and this is not one; `OPTIONS *` is the other shape, and there is nothing
/// here to describe. Both are refused rather than guessed at.
fn request_line(line: &str) -> Result<Request, Response> {
    let malformed = || Response::problem(400, "Bad Request", "the request line is not `METHOD /path HTTP/1.x`");
    let mut parts = line.split(' ');
    let (Some(method), Some(target), Some(version)) = (parts.next(), parts.next(), parts.next()) else {
        return Err(malformed());
    };
    if !version.starts_with("HTTP/1.") || parts.next().is_some() {
        return Err(malformed());
    }
    if !target.starts_with('/') {
        return Err(Response::problem(400, "Bad Request", "only origin-form targets (`/path`) are served"));
    }
    // The query is the page's business, not the server's: every route here is a fixed path,
    // and `/?view=board` is the same document as `/`.
    let path = target.split('?').next().unwrap_or(target);
    Ok(Request { method: method.to_string(), path: path.to_string(), host: None })
}

/// `Name: value`, matched case-insensitively on the name as HTTP requires.
fn header_value(line: &str, name: &str) -> Option<String> {
    let (field, value) = line.split_once(':')?;
    field.trim().eq_ignore_ascii_case(name).then(|| value.trim().to_string())
}

/// The refusal for a head that never finished.
///
/// A read error and a head that ran past the cap are the same thing to the answer: this is
/// not a request. Naming the cap beats a bare "bad request" for the one client that meets it.
fn truncated() -> Response {
    Response::problem(400, "Bad Request", &format!("the request head ended early or ran past {} KiB", MAX_HEAD / 1024))
}

/// Read the request head.
///
/// `Ok(None)` means the client connected and said nothing — a browser's speculative
/// preconnect, or a port check. Answering that with a 400 would be noise about a client that
/// never asked anything, so it is a distinct outcome rather than a refusal.
pub(crate) fn read_head(src: impl Read) -> Result<Option<Request>, Response> {
    let mut reader = BufReader::new(src.take(MAX_HEAD));
    match next_line(&mut reader) {
        Ok(None) => Ok(None),
        Ok(Some(line)) => {
            let mut request = request_line(&line)?;
            request.host = read_host(&mut reader)?;
            Ok(Some(request))
        },
        Err(_) => Err(truncated()),
    }
}

/// Drain the header block up to the blank line, keeping only `Host`.
///
/// The rest is discarded rather than collected: nothing here negotiates content, follows a
/// referer or reads a cookie, and a header map nobody consults is a place for a bug to hide.
fn read_host(reader: &mut impl BufRead) -> Result<Option<String>, Response> {
    let mut host = None;
    loop {
        match next_line(reader) {
            Ok(Some(line)) if line.is_empty() => return Ok(host),
            Ok(Some(line)) => host = header_value(&line, "host").or(host),
            Ok(None) | Err(_) => return Err(truncated()),
        }
    }
}

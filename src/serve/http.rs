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
const MAX_HEAD: usize = 8 * 1024;

/// The body a request may carry, capped.
///
/// A staged batch from the page is a few hundred bytes; anything approaching this is not one.
/// The cap is what stops a client that lies about `Content-Length` from being able to ask this
/// process for an allocation.
const MAX_BODY: usize = 256 * 1024;

/// What routing depends on: the method, the path with any query stripped, the `Host` the
/// client thought it was talking to, and — for the one verb that writes — what it sent.
pub(crate) struct Request {
    pub(crate) method: String,
    pub(crate) path: String,
    pub(crate) host: Option<String>,
    /// Empty for a GET, which is every route but one.
    pub(crate) body: String,
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

    /// A JSON answer, at whatever status the outcome deserves.
    ///
    /// The status and the document both carry the verdict on purpose: `fetch` does not reject
    /// on a 4xx, so a page that only looked at `ok` in the body would be right, and a caller
    /// that only looked at the status would be right too. They must not be able to disagree,
    /// which is why the one function that builds this takes both.
    pub(crate) fn json(code: u16, reason: &'static str, body: String) -> Response {
        Response { code, reason, content_type: "application/json; charset=utf-8", extra: "", body }
    }

    /// A refusal, with the diagnostic as the body: `curl` shows it, and a browser tab that
    /// went to the wrong path says why rather than rendering nothing.
    pub(crate) fn problem(code: u16, reason: &'static str, detail: &str) -> Response {
        Response { code, reason, content_type: "text/plain; charset=utf-8", extra: "", body: format!("{code} {reason}: {detail}\n") }
    }

    /// A 405, carrying the `Allow` the status is required to come with.
    ///
    /// `allow` is the whole header line rather than a method name, because a `&'static str` is
    /// what [`Response::extra`] holds and every caller knows its answer at compile time — the
    /// route decides which methods it has, not the request.
    pub(crate) fn method_not_allowed(method: &str, allow: &'static str) -> Response {
        let permitted = allow.trim_start_matches("Allow: ").trim_end();
        Response { extra: allow, ..Response::problem(405, "Method Not Allowed", &format!("{method} is not served here; this route takes {permitted}")) }
    }

    /// The two `Allow` lines this server has.
    pub(crate) const ALLOW_GET: &'static str = "Allow: GET\r\n";
    pub(crate) const ALLOW_POST: &'static str = "Allow: POST\r\n";

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
///
/// `budget` is what is left of [`MAX_HEAD`], spent as lines are read. Capping *here* rather
/// than by wrapping the whole stream in a `Take` is what lets a body follow: that `Take` would
/// bound the body by the head's allowance, and unwrapping it afterwards would throw away
/// whatever the buffer had already pulled in past the blank line.
fn next_line(src: &mut impl BufRead, budget: &mut usize) -> std::io::Result<Option<String>> {
    let mut line = String::new();
    let read = src.take(*budget as u64).read_line(&mut line)?;
    if read == 0 {
        return Ok(None);
    }
    *budget -= read;
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
    Ok(Request { method: method.to_string(), path: path.to_string(), host: None, body: String::new() })
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
    let mut reader = BufReader::new(src);
    let mut budget = MAX_HEAD;
    match next_line(&mut reader, &mut budget) {
        Ok(None) => Ok(None),
        Ok(Some(line)) => {
            let mut request = request_line(&line)?;
            let headers = read_headers(&mut reader, &mut budget)?;
            request.host = headers.host;
            // Only where one is expected. A GET with a body is legal and meaningless, and
            // reading it would make every ordinary page load wait on a length nobody sent.
            if request.method != "GET" {
                request.body = read_body(&mut reader, headers.length)?;
            }
            Ok(Some(request))
        },
        Err(_) => Err(truncated()),
    }
}

/// The two headers anything here acts on, read out of the block on the way past.
///
/// Everything else is discarded rather than collected: nothing negotiates content, follows a
/// referer or reads a cookie, and a header map nobody consults is a place for a bug to hide.
#[derive(Default)]
struct Headers {
    host: Option<String>,
    length: Option<usize>,
}

/// Drain the header block up to the blank line, keeping the two that matter.
fn read_headers(reader: &mut impl BufRead, budget: &mut usize) -> Result<Headers, Response> {
    let mut headers = Headers::default();
    loop {
        match next_line(reader, budget) {
            Ok(Some(line)) if line.is_empty() => return Ok(headers),
            Ok(Some(line)) => {
                headers.host = header_value(&line, "host").or(headers.host);
                // A length that is not a number is a length this cannot honour, and reading a
                // body without one would be reading until the client hangs up. Left as `None`,
                // which the body read below turns into the refusal.
                headers.length = header_value(&line, "content-length").and_then(|v| v.parse().ok()).or(headers.length);
            },
            Ok(None) | Err(_) => return Err(truncated()),
        }
    }
}

/// Read exactly the bytes the head said were coming.
///
/// **Exactly**, not "until the end": the connection is not framed by anything else, so a read
/// to EOF would sit there until the client closed its own write side, and a short read would
/// hand routing half a document. A body that is not valid UTF-8 is refused here rather than
/// downstream — every route that takes one takes JSON.
fn read_body(reader: &mut impl BufRead, length: Option<usize>) -> Result<String, Response> {
    // No `Content-Length` means no body — that is what it means in a request, and it is what
    // `POST /` with nothing after the blank line sends. A route that needs a body refuses an
    // empty one in its own words, which is a better error than a protocol-level one about a
    // header the sender never meant to omit.
    let Some(length) = length else {
        return Ok(String::new());
    };
    if length > MAX_BODY {
        return Err(Response::problem(413, "Content Too Large", &format!("a body of {length} bytes is past the {MAX_BODY}-byte limit")));
    }
    let mut bytes = vec![0u8; length];
    if reader.read_exact(&mut bytes).is_err() {
        return Err(Response::problem(400, "Bad Request", "the body ended before Content-Length said it would"));
    }
    String::from_utf8(bytes).map_err(|_| Response::problem(400, "Bad Request", "the body is not valid UTF-8"))
}

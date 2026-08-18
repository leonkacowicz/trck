//! `trck serve` — the tracker's page from a live process instead of a file.
//!
//! This is the binary's first listening socket. What that buys, next to `trck html`, is a
//! page rendered from the tracker as it is at the moment of the request rather than as it was
//! when someone last remembered to regenerate a file. What it costs is a process that has to
//! be bound somewhere, and the whole of that answer is **127.0.0.1 and nothing else**: not a
//! default that a flag can widen, because a tracker is a repository's working notes and the
//! surface that serves them should not be reachable from a network at all. The help text says
//! so rather than leaving it implied.
//!
//! **Shutdown is the default disposition, on purpose.** Ctrl-C sends `SIGINT`, nothing here
//! installs a handler, so the kernel terminates the process and closes the listener with it —
//! the port is free the moment the shell prompt comes back. Installing a handler would need a
//! raw `signal()` call, and `unsafe` is forbidden in this crate; it would also have nothing to
//! do. No verb reached from here writes a file, holds a lock, or leaves a temp tree, so there
//! is no state that a graceful path could flush that an abrupt one loses. `tests/serve.rs`
//! asserts the outcome that actually matters: after a `SIGINT`, the port binds again.

mod apply;
mod edits;
mod http;
mod poll;
mod route;
// Each gated on its own: `#[cfg(test)]` reaches the next item and no further, so a list of
// test modules under one attribute compiles all but the first into the shipped binary.
#[cfg(test)]
mod test_apply;
#[cfg(test)]
mod test_edits;
#[cfg(test)]
mod test_http;

use http::Response;
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

/// The default port: T-R-C-K on a telephone keypad, in the range IANA leaves unassigned.
/// Arbitrary is unavoidable; memorable is free.
pub(crate) const DEFAULT_PORT: u16 = 8725;

/// How long a connection may take to send its request or accept its answer.
///
/// A socket that opens and then says nothing would otherwise hold a thread for as long as the
/// process lives, which is how a browser tab left open in the background eventually exhausts
/// the cap below.
const TIMEOUT: Duration = Duration::from_secs(15);

/// Connections served at once. Past this, a request is turned away with a 503 rather than
/// spawning a thread for it: on loopback a browser opens a handful, and anything that opens
/// hundreds is a bug — its own or someone else's — that should not be able to grow this
/// process without bound.
const MAX_LIVE: usize = 64;

/// The `--port` argument as a port, or the sentence that says what a port is.
///
/// `70000` and `nine` are the same mistake to whoever typed it — a value that is not a port —
/// so both get the message naming the range rather than one naming the parse rule the text
/// happened to break. `u16` is what does the deciding; there is no second rule to keep in step.
fn port_from(spec: Option<&str>) -> Result<u16, String> {
    match spec {
        None => Ok(DEFAULT_PORT),
        Some(text) => text.parse().map_err(|_| format!("bad port '{text}' (must be 0-65535; 0 asks the OS to choose one)")),
    }
}

/// Bind loopback, or say why not in a sentence naming the port.
///
/// The busy case is the one worth wording: it is what happens when a `serve` is already
/// running in another terminal, which is the most likely way anyone meets this error, and a
/// bare `Address already in use` leaves them to work out both which port and what to do.
fn bind(port: u16) -> Result<TcpListener, String> {
    TcpListener::bind((Ipv4Addr::LOCALHOST, port)).map_err(|e| match e.kind() {
        std::io::ErrorKind::AddrInUse => {
            format!("port {port} is already in use — stop what is listening there, or pass `--port N` (or `--port 0` to have the OS choose)")
        },
        _ => format!("cannot listen on 127.0.0.1:{port}: {e}"),
    })
}

/// Say where the page is, before the loop that will not return.
///
/// Printed here rather than returned as the verb's output for the obvious reason: the verb's
/// output is printed when it finishes, and this one does not finish. `--port 0` makes it
/// load-bearing rather than decorative — the OS chose the port and this line is the only
/// place it is stated.
fn announce(addr: &SocketAddr) {
    // A closed stdout is not a reason to refuse to serve, and it is already the one io
    // failure this engine treats as success everywhere else.
    let _ = crate::cli::emit(&format!("serving http://{addr}/ — loopback only, Ctrl-C to stop\n"));
}

/// Read one request, answer it, close.
///
/// One request per connection: with `Connection: close` there is no second message to frame,
/// which removes the whole of HTTP's keep-alive bookkeeping from a server that renders a page
/// in more time than it would save.
///
/// Every io error here is dropped deliberately. A client that hangs up halfway through its
/// request, or before reading the answer, has done nothing this process should report or
/// react to — it is the browser closing a preconnect, or a tab being closed mid-load.
fn serve_one(ctx: &crate::discovery::Ctx, stream: &TcpStream) {
    let _ = stream.set_read_timeout(Some(TIMEOUT));
    let _ = stream.set_write_timeout(Some(TIMEOUT));
    let answer = match http::read_head(stream) {
        // The client connected and said nothing. Nothing is the right answer.
        Ok(None) => return,
        Ok(Some(request)) => route::respond(ctx, &request),
        Err(refusal) => refusal,
    };
    let mut out = stream;
    let _ = answer.write_to(&mut out);
}

/// Hand one accepted connection to a thread, or turn it away if too many are in flight.
///
/// The counter is incremented before the thread starts and decremented as its last act, so
/// the cap counts connections being served rather than threads that happen to exist. Relaxed
/// ordering is enough: nothing is published through it, and being off by one at the boundary
/// costs one connection either way.
fn dispatch<'s>(scope: &'s std::thread::Scope<'s, '_>, ctx: &'s crate::discovery::Ctx, live: &'s AtomicUsize, stream: TcpStream) {
    if live.fetch_add(1, Ordering::Relaxed) >= MAX_LIVE {
        live.fetch_sub(1, Ordering::Relaxed);
        let mut out = &stream;
        let _ = Response::problem(503, "Service Unavailable", "too many connections in flight").write_to(&mut out);
        return;
    }
    scope.spawn(move || {
        serve_one(ctx, &stream);
        live.fetch_sub(1, Ordering::Relaxed);
    });
}

/// Accept until the process is signalled.
///
/// A scope rather than detached threads so a connection's thread can borrow the resolved
/// `Ctx` instead of every one of them owning a clone. It never returns, which is what
/// `SIGINT` is for; a thread that finishes inside a scope releases itself, so nothing
/// accumulates while it runs.
///
/// An accept that fails is skipped rather than fatal: `ECONNABORTED` is a client that gave up
/// between the handshake and the accept, and tearing the server down over one would make a
/// port scan a denial of service.
/// The poller shares the scope so that it, too, borrows the resolved `Ctx` — and so that
/// there is one place where every thread this process owns is started.
fn accept_loop(ctx: &crate::discovery::Ctx, listener: &TcpListener, every: Option<Duration>) {
    let live = AtomicUsize::new(0);
    let live = &live;
    std::thread::scope(|scope| {
        if let Some(every) = every {
            scope.spawn(move || poll::run(ctx, every));
        }
        for stream in listener.incoming().flatten() {
            dispatch(scope, ctx, live, stream);
        }
    });
}

/// Serve the tracker until the process is stopped.
pub(crate) fn cmd_serve(ctx: &crate::discovery::Ctx, port: Option<&str>, poll_spec: Option<&str>) -> Result<String, String> {
    // Both arguments are validated before anything is bound, so a typo costs no socket.
    let every = poll::interval_from(poll_spec)?;
    let listener = bind(port_from(port)?)?;
    // Asked of the socket rather than assumed from the argument: `--port 0` means the OS chose,
    // and only the socket knows what it chose.
    let addr = listener.local_addr().map_err(|e| format!("cannot read the address of the listening socket: {e}"))?;
    announce(&addr);
    accept_loop(ctx, &listener, every);
    // Reached only if `incoming()` ends, which it does not. Empty rather than a farewell:
    // the verb's success output would print after a shutdown nobody is watching for.
    Ok(String::new())
}

#[cfg(test)]
mod tests {
    // Tests assert; that is their job. The crate denies unwrap/expect/panic because a
    // malformed tracker must produce a diagnostic rather than a stack trace, but a test
    // that cannot panic cannot fail.
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    /// Loopback and nothing else. The one line of this verb that is a security property
    /// rather than a default, so it is asserted rather than left to the string in `bind`.
    #[test]
    fn the_listener_is_bound_to_loopback_only() {
        let listener = bind(0).expect("binds an ephemeral port");
        let addr = listener.local_addr().expect("has an address");
        assert_eq!(addr.ip(), Ipv4Addr::LOCALHOST, "serve bound something other than 127.0.0.1");
        assert_ne!(addr.port(), 0, "port 0 should resolve to what the OS chose");
    }

    /// A value that is not a port, however it fails to be one, gets the sentence that says
    /// what a port is. Absent means the default, which is what makes `trck serve` a whole
    /// invocation.
    #[test]
    fn a_port_that_is_not_one_is_refused_by_range_rather_than_by_parse_rule() {
        assert_eq!(port_from(None), Ok(DEFAULT_PORT));
        assert_eq!(port_from(Some("9000")), Ok(9000));
        assert_eq!(port_from(Some("0")), Ok(0));
        for bad in ["nine", "70000", "-1", "80.5", ""] {
            let err = port_from(Some(bad)).expect_err(bad);
            assert!(err.contains("0-65535"), "{bad} was refused without saying what a port is: {err}");
            assert!(err.contains(bad), "{bad} was refused without quoting what was typed: {err}");
        }
    }

    /// The diagnostic a second `serve` in a second terminal produces. It has to name the
    /// port: that is the one fact the user needs and the one thing the io error omits.
    #[test]
    fn a_busy_port_names_the_port_rather_than_panicking() {
        let held = bind(0).expect("binds");
        let port = held.local_addr().expect("addr").port();
        let err = bind(port).expect_err("the port is held");
        assert!(err.contains(&port.to_string()), "{err}");
        assert!(err.contains("already in use"), "{err}");
    }
}

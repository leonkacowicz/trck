//! Whether output is coloured, and what colour each thing is.
//!
//! Colour is TTY-gated and honours `NO_COLOR`, so piping to a file or into the
//! conformance runner produces plain text. That is not only politeness — it is what makes
//! rendered output comparable at all, and it is why [`paint`] is the only place that ever
//! emits an escape: a second one would be a second thing to remember to gate.
//!
//! Two palettes, for two different jobs. Status and priority are a *trichrome* — the same
//! few meanings everywhere, so a reader learns them once. Lanes in the dependency gutter
//! are a spread of hues instead, because there the point is telling lanes apart from each
//! other rather than saying what any one of them means.

use crate::config::{self, PRIORITIES};
use std::ffi::OsStr;
use std::io::IsTerminal;

/// A row's leading glyph and its colour. Single-width, so the id column lines up.
///
/// `◇` deliberately leaves the `○◐●` family: that gauge says how far along the work is,
/// and readiness is not a point on it — it says the work is *available*. It is also the
/// one glyph that is not dim, because scanning for it is the point.
pub(crate) fn gutter(status: &str, ready: bool) -> (&'static str, &'static [&'static str]) {
    if ready {
        return ("◇", &["bcyan"]);
    }
    match status {
        config::DONE => ("●", &["green"]),
        config::BACKLOG => ("○", &["dim"]),
        config::IN_PROGRESS | config::IN_REVIEW => ("◐", &["yellow"]),
        _ => ("⏳", &["yellow"]),
    }
}

fn ansi(code: &str) -> &'static str {
    match code {
        "reset" => "\u{1b}[0m",
        "bold" => "\u{1b}[1m",
        "dim" => "\u{1b}[2m",
        "red" => "\u{1b}[31m",
        "green" => "\u{1b}[32m",
        "yellow" => "\u{1b}[33m",
        "blue" => "\u{1b}[34m",
        "magenta" => "\u{1b}[35m",
        "cyan" => "\u{1b}[36m",
        "bgreen" => "\u{1b}[92m",
        "byellow" => "\u{1b}[93m",
        "bblue" => "\u{1b}[94m",
        "bmagenta" => "\u{1b}[95m",
        "bcyan" => "\u{1b}[96m",
        _ => "",
    }
}

/// Whether to emit escape codes at all.
///
/// `NO_COLOR` set to anything, including empty, disables — that is the no-color.org
/// convention. `FORCE_COLOR` set to anything but `0` forces colour on even off a terminal
/// (its companion convention). Otherwise, colour only when stdout is a real terminal —
/// `is_terminal()` is `isatty(1)` from std, so no dependency and no `unsafe` are needed.
pub(crate) fn use_colour() -> bool {
    colour_decision(std::env::var_os("NO_COLOR").is_some(), std::env::var_os("FORCE_COLOR").as_deref(), std::io::stdout().is_terminal())
}

/// The colour gate with its three inputs passed in rather than read from the environment,
/// so the precedence (`NO_COLOR` > `FORCE_COLOR` > isatty) is testable without mutating process
/// state — `set_var` is unsafe in this edition, and the crate forbids unsafe.
fn colour_decision(no_color: bool, force_color: Option<&OsStr>, is_tty: bool) -> bool {
    if no_color {
        return false;
    }
    if force_color.is_some_and(|v| v != OsStr::new("0")) {
        return true;
    }
    is_tty
}

/// Wrap `text` in the given codes, or return it unchanged when colour is off.
pub(crate) fn paint(text: &str, codes: &[&str]) -> String {
    paint_with(use_colour(), text, codes)
}

/// `paint` with the decision passed in rather than read from the environment. Split out
/// so the formatting is testable without mutating process state — `set_var` is unsafe in
/// this edition, and the crate forbids unsafe.
fn paint_with(on: bool, text: &str, codes: &[&str]) -> String {
    if codes.is_empty() || !on {
        return text.to_string();
    }
    let mut out = String::new();
    for c in codes {
        out.push_str(ansi(c));
    }
    out.push_str(text);
    out.push_str(ansi("reset"));
    out
}

pub(crate) fn priority_codes(priority: &str) -> Vec<&'static str> {
    if PRIORITIES.first().is_some_and(|p| *p == priority) {
        vec!["red"]
    } else if PRIORITIES.last().is_some_and(|p| *p == priority) {
        vec!["dim"]
    } else {
        Vec::new()
    }
}

pub(crate) fn status_codes(status: &str) -> Vec<&'static str> {
    match status {
        config::DONE => vec!["green"],
        config::BACKLOG => vec!["dim"],
        _ => vec!["yellow"],
    }
}

/// Rotating palette used to colour graph lanes; each lane keeps one colour for its whole
/// descent so it can be traced through crossings (`deps`). Distinguishing lanes *from each
/// other* is the point, so this is a spread of hues rather than the status trichrome.
pub(crate) const LANE_PALETTE: [&str; 11] = ["red", "green", "yellow", "blue", "magenta", "cyan", "bgreen", "byellow", "bblue", "bmagenta", "bcyan"];

/// The palette slot a lane's owning id lands in. An id is read as one big integer — decimal
/// if it is all digits, otherwise its bytes big-endian — then taken mod the palette length,
/// so the same id always draws the same hue. Only the remainder is ever needed, so it is
/// folded a byte at a time rather than materialising the (unbounded) integer.
pub(crate) fn lane_palette_index(id: &str) -> usize {
    let n = LANE_PALETTE.len();
    // Fold in `usize`: every intermediate is a remainder < n plus one base-256/base-10 digit,
    // so it stays far below `usize::MAX` and never truncates.
    if !id.is_empty() && id.bytes().all(|b| b.is_ascii_digit()) {
        id.bytes().fold(0usize, |acc, b| (acc * 10 + usize::from(b - b'0')) % n)
    } else {
        id.bytes().fold(0usize, |acc, b| (acc * 256 + usize::from(b)) % n)
    }
}

#[cfg(test)]
mod tests {
    // Tests assert; that is their job. The crate denies unwrap/expect/panic because a
    // malformed tracker must produce a diagnostic rather than a stack trace, but a test
    // that cannot panic cannot fail.
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    #[test]
    fn icons_are_one_per_status_and_single_width() {
        // Fixed-width column: no glyph, ready or not, may shift the id.
        for s in config::STATUSES {
            for ready in [false, true] {
                assert_eq!(gutter(s, ready).0.chars().count(), 1, "{s} ready={ready}");
            }
        }
        assert_eq!(gutter("done", false).0, "●");
        assert_eq!(gutter("in-review", false).0, gutter("in-progress", false).0);
        // Availability, not progress: outside ○◐● rather than a fourth degree of fill.
        assert_eq!(gutter("backlog", true).0, "◇");
    }

    #[test]
    fn colour_decision_matches_the_python_gate() {
        let f = OsStr::new;
        // NO_COLOR wins over FORCE_COLOR and over a real tty.
        assert!(!colour_decision(true, Some(f("1")), true));
        // FORCE_COLOR set to anything but "0" forces on, even when piped.
        assert!(colour_decision(false, Some(f("1")), false));
        assert!(colour_decision(false, Some(f("")), false)); // "" != "0" → forced on
        // FORCE_COLOR=0 does not force; the tty check decides.
        assert!(!colour_decision(false, Some(f("0")), false));
        assert!(colour_decision(false, Some(f("0")), true));
        // Unset FORCE_COLOR: follow the terminal.
        assert!(colour_decision(false, None, true));
        assert!(!colour_decision(false, None, false));
    }

    #[test]
    fn colour_off_suppresses_every_escape() {
        // The conformance runner sets NO_COLOR, and this is what makes rendered output
        // comparable at all.
        assert_eq!(paint_with(false, "x", &["red", "bold"]), "x");
        assert_eq!(paint_with(true, "x", &["red"]), "\u{1b}[31mx\u{1b}[0m");
        assert_eq!(paint_with(true, "x", &[]), "x", "no codes, no escapes");
    }

    #[test]
    fn lane_palette_index_matches_the_python_engine() {
        // Oracle values from the Python `paint_lane`: `int.from_bytes(id.encode(), "big")`
        // (or `int(id)` when all-digit) mod len(_LANE_PALETTE).
        for (id, want) in [
            ("sp2rwzx", "green"),
            ("eek4hat", "magenta"),
            ("qktc8z7", "bmagenta"),
            ("bdmgj7r", "magenta"),
            ("2w5panf", "blue"),
            ("a", "bmagenta"),
            ("123", "yellow"),  // all-digit: int("123") % 11 == 2
            ("007", "byellow"), // all-digit, leading zeros: int("007") == 7
        ] {
            assert_eq!(LANE_PALETTE[lane_palette_index(id)], want, "{id}");
        }
    }

    #[test]
    fn every_lane_palette_colour_has_an_escape() {
        for c in LANE_PALETTE {
            assert_ne!(ansi(c), "", "{c} has no ANSI code");
        }
    }
}

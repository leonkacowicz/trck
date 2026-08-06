//! The clock, and the one override that makes it reproducible.
//!
//! `TRCK_NOW` is part of the specification rather than a test hook: the conformance suite
//! compares `index.jsonl` byte for byte, which is only possible if the engine's idea of now
//! is something a caller can fix. Dates are computed rather than formatted by a library —
//! there are no dependencies, and civil-from-days is a dozen lines.

/// The stamp written to `created`/`started`/`closed`.
///
/// `TRCK_NOW` overrides the clock, which is what makes a sequence of commands
/// reproducible for the conformance suite. Read per call, so a fixture can advance it
/// between invocations. A malformed value is an error rather than a fall back to the
/// real clock — falling back would make a fixture pass locally and fail elsewhere for a
/// reason nothing in the output explains.
pub(crate) fn now_utc() -> Result<String, String> {
    match std::env::var("TRCK_NOW") {
        Ok(v) if !v.is_empty() => parse_instant(&v),
        _ => Ok(system_now()),
    }
}

/// Seconds since the Unix epoch, rendered as the engine's canonical stamp.
fn system_now() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    format_epoch(i64::try_from(secs).unwrap_or(0))
}

/// Civil date-time from a Unix timestamp. Written out because the standard library has
/// no calendar; the algorithm is the usual days-from-civil inverse.
// The calendar arithmetic below is Howard Hinnant's days-from-civil algorithm, kept in
// its published single-letter form. Renaming `y`/`m`/`d`/`doe`/`yoe` to something
// "clearer" would make it unverifiable against the reference for no reader's benefit.
#[allow(clippy::many_single_char_names, reason = "matches the published algorithm")]
fn format_epoch(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (h, mi, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
}

/// Accept any ISO-8601 instant and normalise it to the one shape the engine writes.
/// A day-only value is refused: those are a legacy form the engine no longer emits, and
/// expanding one to midnight would reintroduce them through the back door.
#[allow(clippy::many_single_char_names, reason = "matches the published algorithm")]
fn parse_instant(v: &str) -> Result<String, String> {
    let bad = || format!("TRCK_NOW='{v}' is not an ISO-8601 instant (want e.g. 2026-01-01T00:00:00Z)");
    let (date, rest) = v.split_once('T').ok_or_else(|| {
        if v.len() == 10 && v.split('-').count() == 3 { format!("TRCK_NOW='{v}' is a date, not an instant (want e.g. 2026-01-01T00:00:00Z)") } else { bad() }
    })?;
    let nums: Vec<i64> = date.split('-').map(|p| p.parse().map_err(|_| bad())).collect::<Result<_, _>>()?;
    let [y, m, d] = nums[..] else {
        return Err(bad());
    };
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return Err(bad());
    }
    // Offset handling: strip it, then apply it in seconds.
    let (clock, offset) = split_offset(rest).ok_or_else(bad)?;
    let hms: Vec<i64> = clock.split(':').map(|p| p.split('.').next().unwrap_or(p).parse().map_err(|_| bad())).collect::<Result<_, _>>()?;
    let [h, mi, s] = hms[..] else {
        return Err(bad());
    };
    if h > 23 || mi > 59 || s > 60 {
        return Err(bad());
    }
    Ok(format_epoch(days_from_civil(y, m, d) * 86_400 + h * 3600 + mi * 60 + s - offset))
}

/// `(clock, offset_seconds)` from the part after `T`.
fn split_offset(rest: &str) -> Option<(&str, i64)> {
    if let Some(clock) = rest.strip_suffix('Z') {
        return Some((clock, 0));
    }
    for (i, c) in rest.char_indices().skip(1) {
        if c == '+' || c == '-' {
            let (clock, off) = rest.split_at(i);
            let sign = if c == '-' { -1 } else { 1 };
            let (hh, mm) = off[1..].split_once(':')?;
            let h: i64 = hh.parse().ok()?;
            let m: i64 = mm.parse().ok()?;
            return Some((clock, sign * (h * 3600 + m * 60)));
        }
    }
    Some((rest, 0)) // naive: treated as UTC
}

#[allow(clippy::many_single_char_names, reason = "matches the published algorithm")]
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

#[cfg(test)]
mod tests {
    // Tests assert; that is their job. The crate denies unwrap/expect/panic because a
    // malformed tracker must produce a diagnostic rather than a stack trace, but a test
    // that cannot panic cannot fail.
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    #[test]
    fn epoch_formatting_round_trips_known_instants() {
        assert_eq!(format_epoch(0), "1970-01-01T00:00:00Z");
        assert_eq!(format_epoch(1_767_225_600), "2026-01-01T00:00:00Z");
        assert_eq!(format_epoch(951_782_400), "2000-02-29T00:00:00Z");
    }
    #[test]
    fn trck_now_accepts_any_iso_instant_and_normalises_to_utc() {
        assert_eq!(parse_instant("2026-01-01T00:00:00Z").expect("ok"), "2026-01-01T00:00:00Z");
        assert_eq!(parse_instant("2026-01-01T09:00:00+03:00").expect("ok"), "2026-01-01T06:00:00Z");
    }
    #[test]
    fn trck_now_refuses_a_day_only_or_malformed_value() {
        // Refused rather than ignored: falling back to the real clock would make a
        // fixture pass locally and fail elsewhere for no visible reason.
        assert!(parse_instant("2026-01-01").expect_err("refused").contains("not an instant"));
        for bad in ["yesterday", "1735689600", "2026-13-01T00:00:00Z", "x"] {
            assert!(parse_instant(bad).is_err(), "should reject {bad}");
        }
    }
}

//! One row's status transition, and the dates it implies.

use crate::config::{self, is_terminal};
use crate::issue::Issue;
use crate::verbs::now_utc;

/// Apply a status transition and stamp the dates it implies.
///
/// Pure — no filesystem contact — so it is safe wherever the working tree may not be
/// settled: in-memory normalisation, dry runs, merge drivers.
pub(crate) fn apply_status(row: &mut Issue, new_status: &str) -> Result<(), String> {
    if let Some(msg) = config::check_status(new_status) {
        return Err(msg);
    }
    let was_initial = row.status == config::BACKLOG;
    row.status = new_status.to_string();
    if was_initial && new_status != config::BACKLOG && row.started.is_none() {
        row.started = Some(now_utc()?);
    }
    if is_terminal(new_status) {
        if row.closed.is_none() {
            row.closed = Some(now_utc()?);
        }
    } else {
        // Reopening clears the whole closure record. Dropping the timestamp but keeping
        // the resolution would leave a row that is open and yet says *why* it closed —
        // a state `check` rejects, so the verb would be writing an invalid tracker.
        row.closed = None;
        row.resolution = None;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    // Tests assert; that is their job. The crate denies unwrap/expect/panic because a
    // malformed tracker must produce a diagnostic rather than a stack trace, but a test
    // that cannot panic cannot fail.
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;
    use crate::test_graph::issue;

    #[test]
    fn reopening_clears_both_closed_and_resolution() {
        // Leaving a terminal status must clear the whole closure record, not just the
        // timestamp: a row that is 'in-progress' while carrying a resolution is one our own
        // `check` rejects, so keeping it would have the verb write an invalid tracker.
        let mut row = issue("aaaaaaa @done");
        row.closed = Some("2026-01-01T00:00:00Z".into());
        row.resolution = Some("wontfix".into());
        apply_status(&mut row, config::IN_PROGRESS).expect("transition");
        assert_eq!(row.closed, None);
        assert_eq!(row.resolution, None, "resolution must not outlive the closure");
    }

    /// Leaving the initial status stamps `started` once, and a later move does not restamp it
    /// — the first claim is the one that counts.
    #[test]
    fn started_is_stamped_on_first_leaving_backlog_and_not_again() {
        let mut row = issue("aaaaaaa");
        assert_eq!(row.started, None);
        apply_status(&mut row, config::IN_PROGRESS).expect("start");
        let first = row.started.clone();
        assert!(first.is_some());
        apply_status(&mut row, config::IN_REVIEW).expect("review");
        assert_eq!(row.started, first, "started must not be restamped");
    }

    /// Closing stamps `closed`, and closing again keeps the original date.
    #[test]
    fn closed_is_stamped_once_too() {
        let mut row = issue("aaaaaaa @in-progress");
        apply_status(&mut row, config::DONE).expect("done");
        let first = row.closed.clone();
        assert!(first.is_some());
        apply_status(&mut row, config::DONE).expect("done again");
        assert_eq!(row.closed, first);
    }

    /// Returning to the initial status does not stamp `started`, since nothing was started.
    #[test]
    fn moving_back_to_backlog_stamps_nothing() {
        let mut row = issue("aaaaaaa @done");
        apply_status(&mut row, config::BACKLOG).expect("reopen");
        assert_eq!(row.started, None);
        assert_eq!(row.closed, None);
    }

    #[test]
    fn an_unknown_status_is_refused_and_the_row_is_untouched() {
        let mut row = issue("aaaaaaa");
        assert!(apply_status(&mut row, "nonsense").is_err());
        assert_eq!(row.status, config::BACKLOG, "a refused transition must not half-apply");
    }
}

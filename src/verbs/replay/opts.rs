//! Rebuilding a verb's options out of the operation that recorded them.
//!
//! Split from [`super`] because it is a different kind of work: the dispatcher decides *which*
//! verb ran, and this decides what it was given. Keeping them apart is also what keeps the
//! dispatcher a list of one-line arms rather than a function whose shape hides a missing flag.
//!
//! The rule throughout: a flag the op never carried stays **absent**, not empty. `set` treats
//! "not given" and "given as none" as different things — one leaves a field alone and the other
//! clears it — so a rebuild that turned the first into the second would quietly erase a parent.

use super::super::{Op, SetOpts};

/// Every value a repeatable flag carries, in order.
pub(super) fn values<'a>(op: &'a Op, name: &str) -> Vec<&'a str> {
    op.flags.iter().filter(|(n, _)| n == name).filter_map(|(_, v)| v.as_deref()).collect()
}

/// `set`'s options, rebuilt from the flags the op recorded.
///
/// Borrowed straight out of the op, which is why this takes and returns a lifetime rather than
/// building owned values: `SetOpts` is what the CLI hands the verb, and the op outlives the call.
pub(super) fn set_opts(op: &Op) -> SetOpts<'_> {
    SetOpts {
        auto: op.flags.iter().any(|(n, v)| n == "--auto" && v.is_none()),
        priority: op.flag_value("--priority"),
        points: op.flag_value("--points").and_then(|p| p.parse().ok()),
        parent: op.flag_value("--parent"),
        spec: op.flag_value("--spec"),
        review_url: op.flag_value("--review-url"),
        title: op.flag_value("--title"),
        slug: op.flag_value("--slug"),
        fields: values(op, "--field"),
        unset: values(op, "--unset"),
    }
}

#[cfg(test)]
mod tests {
    // Tests assert; that is their job. The crate denies unwrap/expect/panic because a
    // malformed tracker must produce a diagnostic rather than a stack trace, but a test
    // that cannot panic cannot fail.
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    /// Every flag `set` was given comes back, including the switch — which carries no value and
    /// so is the one that a naive rebuild silently drops.
    #[test]
    fn set_options_are_rebuilt_from_the_recorded_flags() {
        let op = Op::new("set")
            .operand("aaaaaaa")
            .switch("--auto", true)
            .flag("--priority", Some("high"))
            .flag("--points", Some("5"))
            .flag("--title", Some("A new title"))
            .repeated("--field", &["assignee=someone", "team=core"])
            .repeated("--unset", &["stale"]);
        let opts = set_opts(&op);
        assert!(opts.auto);
        assert_eq!(opts.priority, Some("high"));
        assert_eq!(opts.points, Some(5));
        assert_eq!(opts.title, Some("A new title"));
        assert_eq!(opts.fields, vec!["assignee=someone", "team=core"]);
        assert_eq!(opts.unset, vec!["stale"]);
    }

    /// A flag the op does not carry stays absent rather than becoming a value — `set` treats
    /// "not given" and "given as none" as different things, and replay must not confuse them.
    #[test]
    fn a_flag_the_op_never_had_stays_absent() {
        let op = Op::new("set").operand("aaaaaaa").flag("--priority", Some("low"));
        let opts = set_opts(&op);
        assert_eq!(opts.parent, None);
        assert_eq!(opts.spec, None);
        assert!(!opts.auto);
        assert!(opts.fields.is_empty());
    }

    /// A repeatable flag keeps every value and their order.
    #[test]
    fn repeated_values_all_come_back() {
        let op = Op::new("label").operand("aaaaaaa").repeated("--add", &["infra", "urgent"]);
        assert_eq!(values(&op, "--add"), vec!["infra", "urgent"]);
        assert!(values(&op, "--remove").is_empty());
    }
}

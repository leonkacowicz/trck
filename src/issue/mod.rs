//! The issue record, and the canonical form written to `index.jsonl`.
//!
//! The shape mirrors the Python engine exactly, because the conformance suite compares
//! `index.jsonl` byte for byte. Three things in particular are contract, not choice:
//!
//! * **Field order.** [`CANON_KEYS`] is the serialisation order, unknown keys after it.
//! * **Defaults are stripped.** A field equal to its default is omitted, so a tracker's
//!   diff shows what someone changed rather than what the writer happened to know.
//! * **Unknown keys survive.** [`Issue::extra`] round-trips anything this engine has
//!   never heard of. That is the guarantee the format version rests on: adding a field
//!   never makes an older engine *wrong*, only ignorant, so it needs no version bump.
//!
//! Reading a row is four steps, one module each: [`row`] collapses the pairs, [`read`] decides
//! what each field is, [`coerce`] turns a value into it or refuses, and [`diagnostic`] words
//! the refusal — whose wording is itself contract. [`write`] is the other direction.

use crate::json::Json;
use std::collections::BTreeMap;

mod coerce;
mod diagnostic;
mod read;
mod row;
mod write;

/// The serialisation order. Exactly the known fields, nothing else.
pub(crate) const CANON_KEYS: &[&str] = &[
    "id",
    "slug",
    "title",
    "status",
    "priority",
    "points",
    "parent",
    "labels",
    "depends_on",
    "spec",
    "review_url",
    "created",
    "started",
    "closed",
    "resolution",
    "manual_status",
];

/// What `new` assigns when no weight is given.
pub(crate) const DEFAULT_POINTS: i64 = 1;

/// Index keys this engine rewrites on read. A custom field may not take one of these
/// names: it would be silently absorbed by the migration on the next load.
pub(crate) const LEGACY_KEYS: &[(&str, &str)] =
    &[("milestone", "it migrates to a label; use `trck label`"), ("pr", "it migrates to `review_url`; use --review-url")];

/// A single issue.
///
/// The five identity/state fields are required; the rest carry defaults. Anything not
/// listed here lands in `extra` and is written back verbatim.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Issue {
    pub(crate) id: String,
    pub(crate) slug: String,
    pub(crate) title: String,
    pub(crate) status: String,
    pub(crate) priority: String,
    pub(crate) points: i64,
    pub(crate) parent: Option<String>,
    pub(crate) labels: Vec<String>,
    pub(crate) depends_on: Vec<String>,
    pub(crate) spec: Option<String>,
    pub(crate) review_url: Option<String>,
    pub(crate) created: Option<String>,
    pub(crate) started: Option<String>,
    pub(crate) closed: Option<String>,
    pub(crate) resolution: Option<String>,
    pub(crate) manual_status: bool,
    /// Unknown keys, verbatim. Sorted, so the canonical form is stable.
    pub(crate) extra: BTreeMap<String, Json>,
}

/// Validate a custom-field key.
///
/// Rejects the built-in names — use their flag or verb — and the legacy names, which
/// [`Issue::from_json`] migrates away on read: a custom field under one of those would be
/// swallowed the next time the index was loaded.
pub(crate) fn check_field_key(key: &str) -> Option<String> {
    if CANON_KEYS.contains(&key) {
        return Some(format!("'{key}' is a built-in field; use its flag/verb, not --field/--unset"));
    }
    if let Some((_, why)) = LEGACY_KEYS.iter().find(|(k, _)| *k == key) {
        return Some(format!("'{key}' is a legacy field name ({why}), not a custom field"));
    }
    if !is_field_key(key) {
        return Some(format!("invalid field key '{key}' (must match [a-z][a-z0-9_-]*)"));
    }
    None
}

fn is_field_key(key: &str) -> bool {
    let mut chars = key.chars();
    match chars.next() {
        Some(c) if c.is_ascii_lowercase() => {},
        _ => return false,
    }
    chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
}

#[cfg(test)]
mod tests {
    // Tests assert; that is their job. The crate denies unwrap/expect/panic because a
    // malformed tracker must produce a diagnostic rather than a stack trace, but a test
    // that cannot panic cannot fail.
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    #[test]
    fn legacy_names_are_not_available_as_custom_fields() {
        for (key, hint) in [("pr", "review_url"), ("milestone", "label")] {
            let msg = check_field_key(key).expect("should reject");
            assert!(msg.contains("legacy"), "{msg}");
            assert!(msg.contains(hint), "{msg}");
        }
    }

    #[test]
    fn built_in_names_are_not_available_as_custom_fields() {
        assert!(check_field_key("status").expect("rejected").contains("built-in"));
        assert!(check_field_key("Assignee").is_some());
        assert_eq!(check_field_key("assignee"), None);
    }

    /// Every canonical key is refused as a custom field, not just the ones someone thought
    /// to test — a new field added to `CANON_KEYS` gets this for free.
    #[test]
    fn no_canonical_key_can_be_taken_as_a_custom_field() {
        for key in CANON_KEYS {
            assert!(check_field_key(key).is_some(), "{key} was allowed as a custom field");
        }
    }

    /// The key grammar, at its edges: must start with a lowercase letter, may then carry
    /// digits, underscores and dashes, and nothing else.
    #[test]
    fn a_field_key_must_match_the_documented_grammar() {
        for ok in ["a", "assignee", "due_date", "x-1", "a0"] {
            assert_eq!(check_field_key(ok), None, "{ok} should be allowed");
        }
        for no in ["", "1a", "_a", "-a", "A", "aB", "a b", "a.b", "a:b", "café"] {
            assert!(check_field_key(no).is_some(), "{no:?} should be refused");
        }
    }
}

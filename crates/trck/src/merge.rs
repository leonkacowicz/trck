//! Row-wise 3-way merge of `index.jsonl`.
//!
//! A git merge driver receives three files — `%O` (common ancestor), `%A`, `%B` — and
//! **cannot** determine which side is the user's. `%A` is simply whatever is checked out at
//! that moment: `git merge main` from a feature branch and `git rebase main` from that same
//! branch assign the operands in opposite order. So every rule here is either symmetric in
//! `(a, b)` or derived from the base, and none may branch on "ours". The tests assert that
//! symmetry by running cases both ways round.
//!
//! The base is what makes that sufficient: `base -> side` is the transaction that produced
//! that side, so "who changed what" is recoverable without knowing whose change it was.
//!
//! Rows are merged as raw JSON maps rather than typed [`Issue`]s, mirroring the Python
//! engine: a half-merged row can hold a combination no constructor would accept, and
//! validating it only at the end is what lets the conflict be *reported* rather than
//! becoming a parse error with no explanation.

use crate::issue::{CANON_KEYS, Issue};
use crate::json::Json;
use std::collections::{BTreeMap, BTreeSet};

/// Sets, merged as `(base + additions) - removals`, so a deliberate removal on one side is
/// not resurrected by the other side's untouched copy.
const SET_FIELDS: &[&str] = &["labels", "depends_on"];

/// Earliest wins. `min` is commutative, so this is symmetric for free.
const MIN_FIELDS: &[&str] = &["created", "started"];

/// `(status, closed, resolution)` is maintained as a unit by the mutating verbs, which clear
/// both dates on any move to a non-terminal status. Merging its members independently
/// synthesises rows no verb can write — and does so even when no single field diverges, so a
/// per-field rule never catches it. Merge the tuple whole or conflict.
const TUPLE_FIELDS: &[&str] = &["status", "closed", "resolution"];

/// The ids named by a conflict list. Messages are formatted `#<id>: …` here, so the parsing
/// lives next to the formatting rather than at the call site.
pub(crate) fn conflict_ids(conflicts: &[String]) -> BTreeSet<String> {
    conflicts
        .iter()
        .filter_map(|c| {
            let rest = c.strip_prefix('#')?;
            let end = rest.find(':')?;
            Some(rest[..end].to_string())
        })
        .collect()
}

/// A row as a field map. `None` and an absent key are the same thing here, as in Python.
type Row = BTreeMap<String, Json>;

fn row_of(v: &Json) -> Row {
    match v {
        Json::Object(pairs) => pairs.iter().filter(|(_, v)| !matches!(v, Json::Null)).map(|(k, v)| (k.clone(), v.clone())).collect(),
        _ => Row::new(),
    }
}

fn by_id(rows: &[Json]) -> BTreeMap<String, Row> {
    rows.iter()
        .filter_map(|r| {
            let row = row_of(r);
            let id = row.get("id")?.as_str()?.to_string();
            Some((id, row))
        })
        .collect()
}

fn strings(v: Option<&Json>) -> BTreeSet<String> {
    match v {
        Some(Json::Array(items)) => items.iter().filter_map(|i| i.as_str().map(str::to_string)).collect(),
        _ => BTreeSet::new(),
    }
}

fn set_merge(base: Option<&Json>, a: Option<&Json>, b: Option<&Json>) -> Json {
    let (base_s, a_s, b_s) = (strings(base), strings(a), strings(b));
    let mut out = base_s.clone();
    out.extend(a_s.difference(&base_s).cloned());
    out.extend(b_s.difference(&base_s).cloned());
    for gone in base_s.difference(&a_s).chain(base_s.difference(&b_s)).cloned().collect::<Vec<_>>() {
        out.remove(&gone);
    }
    Json::Array(out.into_iter().map(Json::String).collect())
}

fn min_merge(a: Option<&Json>, b: Option<&Json>) -> Option<Json> {
    let pick = |v: Option<&Json>| v.and_then(|x| x.as_str()).map(str::to_string);
    match (pick(a), pick(b)) {
        (Some(x), Some(y)) => Some(Json::String(x.min(y))),
        (Some(x), None) | (None, Some(x)) => Some(Json::String(x)),
        (None, None) => None,
    }
}

/// Render two competing values in a fixed order.
///
/// The order must not depend on which operand they arrived in: `%A`/`%B` swap between
/// integration directions, so an operand-ordered message would read differently for the same
/// underlying disagreement — and the wording deliberately avoids ours/theirs for the same
/// reason.
fn pair(x: Option<&Json>, y: Option<&Json>) -> String {
    let mut both = [repr(x), repr(y)];
    both.sort();
    format!("{} on one side and {} on the other", both[0], both[1])
}

/// A value as Python's `repr` would render it, which is what the messages were written
/// against: `None` for absent, `'quoted'` for a string, bare otherwise.
fn repr(v: Option<&Json>) -> String {
    match v {
        None | Some(Json::Null) => "None".to_string(),
        Some(Json::String(s)) => format!("'{s}'"),
        Some(Json::Bool(true)) => "True".to_string(),
        Some(Json::Bool(false)) => "False".to_string(),
        Some(other) => other.to_json(),
    }
}

/// Standard 3-way: one side changed → take it; both changed alike → fine; both changed
/// differently → conflict, and keep the base so the result stays symmetric (picking a side
/// would make the output depend on operand order).
fn scalar_merge(iid: &str, field: &str, base: Option<&Json>, a: Option<&Json>, b: Option<&Json>, conflicts: &mut Vec<String>) -> Option<Json> {
    if a == b {
        return a.cloned();
    }
    if a == base {
        return b.cloned();
    }
    if b == base {
        return a.cloned();
    }
    conflicts.push(format!("#{iid}: {field} is {}", pair(a, b)));
    base.cloned()
}

fn tuple_of(row: &Row) -> Vec<Option<Json>> {
    TUPLE_FIELDS.iter().map(|f| row.get(*f).cloned()).collect()
}

/// 3-way merge two sets of index rows keyed by id.
///
/// Returns `(rows, conflicts)`. A non-empty `conflicts` means the caller must not treat the
/// result as resolved — the rows are still returned (holding base values where a field
/// conflicted) so a caller can show context, but the merge has failed. Messages never say
/// ours/theirs: those words mean opposite things depending on the integration direction, so
/// they would be wrong half the time.
pub(crate) fn merge_rows(base_rows: &[Json], a_rows: &[Json], b_rows: &[Json]) -> Result<(Vec<Issue>, Vec<String>), String> {
    let (base, a, b) = (by_id(base_rows), by_id(a_rows), by_id(b_rows));
    let mut conflicts: Vec<String> = Vec::new();
    let mut merged: BTreeMap<String, Row> = BTreeMap::new();

    let ids: BTreeSet<&String> = a.keys().chain(b.keys()).collect();
    for iid in ids {
        let (ra, rb) = (a.get(iid), b.get(iid));
        let in_base = base.contains_key(iid);
        // Deleted on one side. Honour the deletion if the other side left it alone; a
        // delete-vs-modify is a genuine disagreement.
        if ra.is_none() || rb.is_none() {
            let Some(present) = ra.or(rb) else { continue };
            if in_base && base.get(iid) != Some(present) {
                conflicts.push(format!("#{iid}: removed on one side and modified on the other"));
                merged.insert(iid.clone(), present.clone());
            } else if in_base {
                continue; // unchanged on the surviving side -> the deletion wins
            } else {
                merged.insert(iid.clone(), present.clone()); // created on one side only
            }
            continue;
        }
        let (Some(ra), Some(rb)) = (ra, rb) else {
            continue;
        };
        let empty = Row::new();
        let row = merge_one(iid, base.get(iid).unwrap_or(&empty), ra, rb, &mut conflicts);
        merged.insert(iid.clone(), row);
    }

    drop_derived_parent_conflicts(&merged, &mut conflicts);

    let mut rows = Vec::new();
    for row in merged.values() {
        let obj = Json::Object(row.iter().map(|(k, v)| (k.clone(), v.clone())).collect());
        rows.push(Issue::from_json(&obj).map_err(|e| format!("merged row: {e}"))?);
    }
    Ok((rows, conflicts))
}

/// A parent's status and points are **derived**, so a divergence there is not two people
/// disagreeing — it is two sides having recomputed from different child sets. Drop those
/// conflicts; the caller re-derives. Leaves keep the real rule.
fn drop_derived_parent_conflicts(merged: &BTreeMap<String, Row>, conflicts: &mut Vec<String>) {
    let parents: BTreeSet<String> = merged.values().filter_map(|r| r.get("parent").and_then(Json::as_str).map(str::to_string)).collect();
    if parents.is_empty() {
        return;
    }
    conflicts
        .retain(|c| !parents.iter().any(|p| c.starts_with(&format!("#{p}:")) && (c.contains(" status ") || c.contains(" points ") || c.contains("lifecycle"))));
}

/// Merge one row present on both sides, field by field, using `base` to tell which side
/// changed what. An absent base means the id was created independently on both sides, which
/// makes every differing field a conflict — correct, and vanishingly rare with random ids.
fn merge_one(iid: &str, base: &Row, a: &Row, b: &Row, conflicts: &mut Vec<String>) -> Row {
    let mut out = Row::new();
    out.insert("id".into(), Json::String(iid.to_string()));

    let (ta, tb, tbase) = (tuple_of(a), tuple_of(b), tuple_of(base));
    let chosen = if ta == tb {
        ta
    } else if ta == tbase {
        tb
    } else if tb == tbase {
        ta
    } else {
        // Named by content, not by side: "one side"/"the other" reads correctly whichever
        // direction produced the merge.
        conflicts.push(format!("#{iid}: lifecycle status is {} (status/closed/resolution merge as a unit)", pair(ta[0].as_ref(), tb[0].as_ref())));
        tbase
    };
    for (field, value) in TUPLE_FIELDS.iter().zip(chosen) {
        if let Some(v) = value {
            out.insert((*field).to_string(), v);
        }
    }

    for field in CANON_KEYS {
        if *field == "id" || TUPLE_FIELDS.contains(field) {
            continue;
        }
        let (va, vb, vbase) = (a.get(*field), b.get(*field), base.get(*field));
        let merged = if SET_FIELDS.contains(field) {
            Some(set_merge(vbase, va, vb))
        } else if MIN_FIELDS.contains(field) {
            min_merge(va, vb)
        } else {
            scalar_merge(iid, field, vbase, va, vb, conflicts)
        };
        if let Some(v) = merged {
            out.insert((*field).to_string(), v);
        }
    }

    // Custom fields merge per key with the same scalar rule, so a branch adding `assignee`
    // and another adding `component` keeps both.
    let extra: BTreeSet<&String> = a.keys().chain(b.keys()).chain(base.keys()).filter(|k| !CANON_KEYS.contains(&k.as_str())).collect();
    for key in extra {
        let merged = scalar_merge(iid, &format!("field {key}"), base.get(key), a.get(key), b.get(key), conflicts);
        if let Some(v) = merged {
            out.insert(key.clone(), v);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    // Tests assert; that is their job. The crate denies unwrap/expect/panic because a
    // malformed tracker must produce a diagnostic rather than a stack trace, but a test
    // that cannot panic cannot fail.
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    fn row(id: &str, extra: &str) -> Json {
        let body = if extra.is_empty() { String::new() } else { format!(", {extra}") };
        crate::json::parse(&format!(
            r#"{{"id": "{id}", "slug": "s", "title": "T", "status": "backlog",
                "priority": "medium"{body}}}"#
        ))
        .expect("fixture parses")
    }

    fn merge(base: &[Json], a: &[Json], b: &[Json]) -> (Vec<Issue>, Vec<String>) {
        merge_rows(base, a, b).expect("merges")
    }

    /// The invariant the whole design rests on: `%A`/`%B` are handed over in opposite order
    /// by `git merge` and `git rebase` on the same branch, so a rule that is not symmetric
    /// silently produces a different result depending on how you integrated.
    fn assert_symmetric(base: &[Json], a: &[Json], b: &[Json]) {
        let (rows_ab, conf_ab) = merge(base, a, b);
        let (rows_ba, conf_ba) = merge(base, b, a);
        let canon = |rows: &[Issue]| rows.iter().map(|r| r.to_canonical().to_json()).collect::<Vec<_>>();
        assert_eq!(canon(&rows_ab), canon(&rows_ba), "rows differ by operand order");
        assert_eq!(conf_ab, conf_ba, "conflicts differ by operand order");
    }

    #[test]
    fn disjoint_creations_keep_both_rows() {
        let (rows, conflicts) = merge(&[], &[row("aaaaaaa", "")], &[row("bbbbbbb", "")]);
        assert!(conflicts.is_empty(), "{conflicts:?}");
        assert_eq!(rows.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(), ["aaaaaaa", "bbbbbbb"]);
        assert_symmetric(&[], &[row("aaaaaaa", "")], &[row("bbbbbbb", "")]);
    }

    #[test]
    fn a_lifecycle_divergence_conflicts_as_one_unit() {
        let base = [row("aaaaaaa", "")];
        let a = [row("aaaaaaa", r#""status": "ongoing""#)];
        let b = [row("aaaaaaa", r#""status": "done", "closed": "2026-01-01T00:00:00Z""#)];
        let (_, conflicts) = merge(&base, &a, &b);
        assert_eq!(conflicts.len(), 1, "{conflicts:?}");
        assert!(conflicts[0].contains("lifecycle status"), "{conflicts:?}");
        // Neither side is named: those words reverse between merge and rebase.
        for word in ["ours", "theirs", "yours"] {
            assert!(!conflicts[0].to_lowercase().contains(word), "{conflicts:?}");
        }
        assert_symmetric(&base, &a, &b);
    }

    #[test]
    fn one_sided_lifecycle_change_is_taken_not_conflicted() {
        let base = [row("aaaaaaa", "")];
        let a = [row("aaaaaaa", r#""status": "ongoing""#)];
        let (rows, conflicts) = merge(&base, &a, &base);
        assert!(conflicts.is_empty(), "{conflicts:?}");
        assert_eq!(rows[0].status, "ongoing");
        assert_symmetric(&base, &a, &base);
    }

    #[test]
    fn a_removal_on_one_side_wins_when_the_other_left_it_alone() {
        let base = [row("aaaaaaa", ""), row("bbbbbbb", "")];
        let a = [row("aaaaaaa", "")];
        let (rows, conflicts) = merge(&base, &a, &base);
        assert!(conflicts.is_empty(), "{conflicts:?}");
        assert_eq!(rows.len(), 1, "the deletion should have won");
        assert_symmetric(&base, &a, &base);
    }

    #[test]
    fn a_removal_against_a_modification_conflicts() {
        let base = [row("aaaaaaa", ""), row("bbbbbbb", "")];
        let a = [row("aaaaaaa", "")]; // removed bbbbbbb
        let b = [row("aaaaaaa", ""), row("bbbbbbb", r#""priority": "high""#)];
        let (_, conflicts) = merge(&base, &a, &b);
        assert_eq!(conflicts.len(), 1, "{conflicts:?}");
        assert!(conflicts[0].contains("removed on one side"), "{conflicts:?}");
        assert_symmetric(&base, &a, &b);
    }

    #[test]
    fn label_sets_union_additions_and_honour_removals() {
        let base = [row("aaaaaaa", r#""labels": ["keep", "drop"]"#)];
        let a = [row("aaaaaaa", r#""labels": ["keep", "drop", "from-a"]"#)];
        let b = [row("aaaaaaa", r#""labels": ["keep", "from-b"]"#)]; // dropped "drop"
        let (rows, conflicts) = merge(&base, &a, &b);
        assert!(conflicts.is_empty(), "{conflicts:?}");
        assert_eq!(rows[0].labels, ["from-a", "from-b", "keep"]);
        assert_symmetric(&base, &a, &b);
    }

    #[test]
    fn created_takes_the_earliest_of_the_two() {
        let base = [row("aaaaaaa", "")];
        let a = [row("aaaaaaa", r#""created": "2026-05-05T00:00:00Z""#)];
        let b = [row("aaaaaaa", r#""created": "2026-01-01T00:00:00Z""#)];
        let (rows, conflicts) = merge(&base, &a, &b);
        assert!(conflicts.is_empty(), "{conflicts:?}");
        assert_eq!(rows[0].created.as_deref(), Some("2026-01-01T00:00:00Z"));
        assert_symmetric(&base, &a, &b);
    }

    #[test]
    fn custom_fields_merge_per_key_so_both_branches_additions_survive() {
        let base = [row("aaaaaaa", "")];
        let a = [row("aaaaaaa", r#""assignee": "ada""#)];
        let b = [row("aaaaaaa", r#""component": "core""#)];
        let (rows, conflicts) = merge(&base, &a, &b);
        assert!(conflicts.is_empty(), "{conflicts:?}");
        assert_eq!(rows[0].extra.get("assignee").and_then(Json::as_str), Some("ada"));
        assert_eq!(rows[0].extra.get("component").and_then(Json::as_str), Some("core"));
        assert_symmetric(&base, &a, &b);
    }

    #[test]
    fn a_derived_parent_divergence_is_not_a_conflict() {
        // Both sides recomputed the epic's status from different child sets. That is not two
        // people disagreeing, so it must not surface as a conflict the user has to resolve.
        let base = [row("aaaaaaa", ""), row("bbbbbbb", r#""parent": "aaaaaaa""#)];
        let a = [row("aaaaaaa", r#""status": "ongoing""#), row("bbbbbbb", r#""parent": "aaaaaaa", "status": "ongoing""#)];
        let b = [
            row("aaaaaaa", r#""status": "done", "closed": "2026-01-01T00:00:00Z""#),
            row("bbbbbbb", r#""parent": "aaaaaaa", "status": "done", "closed": "2026-01-01T00:00:00Z""#),
        ];
        let (_, conflicts) = merge(&base, &a, &b);
        let on_parent: Vec<&String> = conflicts.iter().filter(|c| c.starts_with("#aaaaaaa:")).collect();
        assert!(on_parent.is_empty(), "derived parent conflict surfaced: {on_parent:?}");
    }

    #[test]
    fn conflict_ids_reads_the_ids_back_out_of_the_messages() {
        let conflicts = vec!["#aaaaaaa: lifecycle status is 'a' on one side and 'b' on the other".to_string(), "not a row message".to_string()];
        assert_eq!(conflict_ids(&conflicts), BTreeSet::from(["aaaaaaa".to_string()]));
    }
}

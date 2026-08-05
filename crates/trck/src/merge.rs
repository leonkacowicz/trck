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
        Json::Object(pairs) => pairs
            .iter()
            .filter(|(_, v)| !matches!(v, Json::Null))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect(),
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
        Some(Json::Array(items)) => items
            .iter()
            .filter_map(|i| i.as_str().map(str::to_string))
            .collect(),
        _ => BTreeSet::new(),
    }
}

fn set_merge(base: Option<&Json>, a: Option<&Json>, b: Option<&Json>) -> Json {
    let (base_s, a_s, b_s) = (strings(base), strings(a), strings(b));
    let mut out = base_s.clone();
    out.extend(a_s.difference(&base_s).cloned());
    out.extend(b_s.difference(&base_s).cloned());
    for gone in base_s
        .difference(&a_s)
        .chain(base_s.difference(&b_s))
        .cloned()
        .collect::<Vec<_>>()
    {
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
fn scalar_merge(
    iid: &str,
    field: &str,
    base: Option<&Json>,
    a: Option<&Json>,
    b: Option<&Json>,
    conflicts: &mut Vec<String>,
) -> Option<Json> {
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
pub(crate) fn merge_rows(
    base_rows: &[Json],
    a_rows: &[Json],
    b_rows: &[Json],
) -> Result<(Vec<Issue>, Vec<String>), String> {
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
                conflicts.push(format!(
                    "#{iid}: removed on one side and modified on the other"
                ));
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
    let parents: BTreeSet<String> = merged
        .values()
        .filter_map(|r| r.get("parent").and_then(Json::as_str).map(str::to_string))
        .collect();
    if parents.is_empty() {
        return;
    }
    conflicts.retain(|c| {
        !parents.iter().any(|p| {
            c.starts_with(&format!("#{p}:"))
                && (c.contains(" status ") || c.contains(" points ") || c.contains("lifecycle"))
        })
    });
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
        conflicts.push(format!(
            "#{iid}: lifecycle status is {} (status/closed/resolution merge as a unit)",
            pair(ta[0].as_ref(), tb[0].as_ref())
        ));
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
    let extra: BTreeSet<&String> = a
        .keys()
        .chain(b.keys())
        .chain(base.keys())
        .filter(|k| !CANON_KEYS.contains(&k.as_str()))
        .collect();
    for key in extra {
        let merged = scalar_merge(
            iid,
            &format!("field {key}"),
            base.get(key),
            a.get(key),
            b.get(key),
            conflicts,
        );
        if let Some(v) = merged {
            out.insert(key.clone(), v);
        }
    }
    out
}

//! Human-facing rendering: id emphasis, the blocking and demand annotations, and the
//! one-line row format the read verbs share.
//!
//! Which colour anything is — and whether there is any colour at all — lives in
//! [`colour`], re-exported here so the read verbs have one rendering import rather than
//! two.

mod colour;
pub(crate) use colour::{LANE_PALETTE, gutter, lane_palette_index, paint, priority_codes, status_codes};

use crate::config::is_terminal;
use crate::graph::Graph;
use crate::issue::{CANON_KEYS, Issue};
use std::collections::BTreeMap;

/// Each id mapped to the length of its shortest prefix that identifies it uniquely —
/// git-short-hash style, the fewest characters you would have to type. When an id is
/// itself a prefix of another, no shorter unique prefix exists, so its full length is
/// used.
pub(crate) fn unique_prefix_lens<'a>(ids: impl IntoIterator<Item = &'a str>) -> BTreeMap<String, usize> {
    let mut uniq: Vec<&str> = ids.into_iter().collect();
    uniq.sort_unstable();
    uniq.dedup();
    let shared = |a: &str, b: &str| a.chars().zip(b.chars()).take_while(|(x, y)| x == y).count();
    let mut out = BTreeMap::new();
    for (i, id) in uniq.iter().enumerate() {
        let before = i.checked_sub(1).map_or(0, |j| shared(uniq[j], id));
        let after = uniq.get(i + 1).map_or(0, |n| shared(id, n));
        let need = before.max(after) + 1;
        out.insert((*id).to_string(), need.min(id.chars().count()).max(1));
    }
    out
}

/// An id with its unique prefix emphasised and the rest dimmed. `hash` prepends `#`,
/// which is the row and graph form; `show` wants the bare id.
pub(crate) fn hl_id(id: &str, abbrev: Option<&BTreeMap<String, usize>>, hash: bool) -> String {
    let head = if hash { "#" } else { "" };
    if let Some(cut) = abbrev.and_then(|a| a.get(id)) {
        let cut = (*cut).min(id.len());
        let (pre, rest) = id.split_at(cut);
        let mut out = format!("{head}{}", paint(pre, &["bold"]));
        if !rest.is_empty() {
            out.push_str(&paint(rest, &["dim"]));
        }
        return out;
    }
    paint(&format!("{head}{id}"), &["bold"])
}

/// The dim `needs #… blocks #…` suffix explaining why a row is waiting, and what is
/// waiting on it.
///
/// `needs` lists non-terminal blockers, an inherited one tagged `(via #author)` so the
/// note never implies the edge was authored here — that is where `dep --remove` goes.
/// It is spelled out only when no row between this one and the author is itself being
/// printed: where such a row exists it already carries the note, and restating it down
/// every child is noise.
pub(crate) fn block_annotations(g: &Graph, id: &str, on_screen: &[String]) -> String {
    let spine = g.ancestors_of(id);
    let carried_above = |author: &str| {
        for a in &spine {
            if on_screen.contains(a) {
                return true;
            }
            if a == author {
                break;
            }
        }
        false
    };

    let mut parts: Vec<String> = Vec::new();
    let mut needs: Vec<String> = Vec::new();
    for author in std::iter::once(id.to_string()).chain(g.ancestors_of(id)) {
        for target in g.requires_of(&author) {
            if needs.iter().any(|n| n.contains(&format!("#{target}"))) {
                continue; // a target reached twice keeps its nearest author
            }
            if g.get(&target).is_some_and(|r| is_terminal(&r.status)) {
                continue; // a done blocker drops off; the block is cleared
            }
            if author == id {
                needs.push(format!("#{target}"));
            } else if !carried_above(&author) {
                needs.push(format!("#{target} (via #{author})"));
            }
        }
    }
    if !needs.is_empty() {
        parts.push(format!("needs {}", needs.join(" ")));
    }
    // `blocks` stays at the authored altitude rather than mirroring the lifting: those
    // dependents' subtrees inherit the wait, and are exactly the rows whose `needs`
    // reads `(via #…)`.
    if !g.get(id).is_some_and(|r| is_terminal(&r.status)) {
        let blocks = g.dependents_of(id);
        if !blocks.is_empty() {
            parts.push(format!("blocks {}", blocks.iter().map(|d| format!("#{d}")).collect::<Vec<_>>().join(" ")));
        }
    }
    if parts.is_empty() { String::new() } else { paint(&format!(" {}", parts.join("  ")), &["dim"]) }
}

/// The ` ↑<priority>(#id)` suffix naming why a row outranks its own priority: the
/// highest-priority issue waiting on it, coloured as that priority. Empty when the row is
/// its own maximum, which most rows are.
///
/// `ready` sorts by the demand cone, so without this a `medium` row sits above a `high`
/// one with nothing on screen to explain it. It rides the same trailing slot `list` uses
/// for its `needs`/`blocks` notes rather than widening the priority column.
pub(crate) fn demand_annotation(g: &Graph, id: &str, abbrev: Option<&BTreeMap<String, usize>>) -> String {
    let Some(src) = g.demand_source(id) else {
        return String::new();
    };
    let Some(row) = g.get(&src) else {
        return String::new();
    };
    format!("  {}({})", paint(&format!("↑{}", row.priority), &priority_codes(&row.priority)), hl_id(&src, abbrev, true))
}

/// The trailing note a view attaches to each row. `list` explains what a row is waiting
/// on; `ready` explains why it ranks where it does. They occupy the same slot, and no
/// view wants both.
#[derive(PartialEq, Eq)]
pub(crate) enum Annotation {
    None,
    Blocking,
    Demand,
}

/// Everything a row needs beyond the issue itself.
pub(crate) struct RowOpts<'a> {
    pub(crate) prefix: Option<&'a BTreeMap<String, String>>,
    pub(crate) dim: &'a [String],
    pub(crate) on_screen: Vec<String>,
    /// Which trailing note to attach, if any.
    pub(crate) annotate: Annotation,
    pub(crate) progress: bool,
    pub(crate) show_fields: Vec<String>,
    pub(crate) abbrev: Option<BTreeMap<String, usize>>,
}

/// Render issues as aligned one-line summaries, shared by `list` and `ready`.
pub(crate) fn render_rows(g: &Graph, rows: &[&Issue], opts: &RowOpts) -> Vec<String> {
    let sw = rows.iter().map(|r| r.status.chars().count()).max().unwrap_or(0);
    let pw = rows.iter().map(|r| r.priority.chars().count()).max().unwrap_or(0);
    let mut out = Vec::new();
    for r in rows {
        let pre = opts.prefix.and_then(|p| p.get(&r.id)).cloned().unwrap_or_default();
        let prog = if opts.progress { g.progress_pct(&r.id).map_or_else(String::new, |p| format!(" {p}%")) } else { String::new() };
        let mut tags: Vec<String> = Vec::new();
        // Derived, not declared: an issue with children *is* an epic. As a stored kind
        // the two drifted, and nothing ever stopped them.
        if !g.children_of(&r.id).is_empty() {
            tags.push("EPIC".into());
        }
        if let Some(parent) = &r.parent
            && pre.is_empty()
        {
            tags.push(format!("↳{parent}")); // the connector shows it when nested
        }
        tags.extend(r.labels.iter().cloned());
        let plain_tags = if tags.is_empty() { String::new() } else { format!(" [{}]", tags.join(" ")) };
        let ann = match opts.annotate {
            Annotation::None => String::new(),
            Annotation::Blocking => block_annotations(g, &r.id, &opts.on_screen),
            Annotation::Demand => demand_annotation(g, &r.id, opts.abbrev.as_ref()),
        };
        let fsuf = field_suffix(r, &opts.show_fields);

        // The glyph carries readiness on its own; the status word beside it stays the
        // status word, coloured by status like every other row's.
        let (icon, icodes) = gutter(&r.status, g.is_ready(&r.id));
        if opts.dim.contains(&r.id) {
            // Ancestor context: the whole line dims, with no per-field colour.
            let body = format!("{icon} #{} {:<sw$}  {:<pw$}  {pre}{}{prog}{plain_tags}", r.id, r.status, r.priority, r.title);
            out.push(format!("{}{ann}{fsuf}", paint(&body, &["dim"])));
            continue;
        }
        out.push(format!(
            "{} {} {}  {}  {pre}{}{}{}{ann}{fsuf}",
            paint(icon, icodes),
            hl_id(&r.id, opts.abbrev.as_ref(), true),
            paint(&format!("{:<sw$}", r.status), &status_codes(&r.status)),
            paint(&format!("{:<pw$}", r.priority), &priority_codes(&r.priority)),
            r.title,
            paint(&prog, &["dim"]),
            paint(&plain_tags, &["dim"]),
        ));
    }
    out
}

/// `key=value` columns for `--show-field`. A built-in field is showable too, not only a
/// custom one; an unset or empty value contributes no column.
fn field_suffix(r: &Issue, show_fields: &[String]) -> String {
    if show_fields.is_empty() {
        return String::new();
    }
    let segs: Vec<String> = show_fields.iter().filter_map(|name| field_value(r, name).map(|v| format!("{name}={v}"))).collect();
    if segs.is_empty() { String::new() } else { format!("  {}", paint(&segs.join(" "), &["dim"])) }
}

/// A Python list literal. `label` and `dep` echo one back and `show` prints one, and the
/// conformance suite compares stdout literally — so the bracket-and-quote style is a
/// contract, not an accident of the first implementation.
pub(crate) fn python_list(items: &[String]) -> String {
    format!("[{}]", items.iter().map(|s| format!("'{s}'")).collect::<Vec<_>>().join(", "))
}

/// One field value, built-in or custom, or `None` when the field is genuinely absent.
///
/// An **empty string is a value**, not an absence: `--field note=` sets one and the index
/// keeps it, so `show` must display it and a `--field note=` filter must match it. That is
/// the difference from [`field_value`], which additionally drops empties because a column
/// showing `note=` for every row carries no information.
pub(crate) fn field_value_raw(r: &Issue, name: &str) -> Option<String> {
    match name {
        other if !CANON_KEYS.contains(&other) => r.extra.get(other).and_then(|v| match v {
            crate::json::Json::String(s) => Some(s.clone()),
            crate::json::Json::Null => None,
            v => Some(v.to_json()),
        }),
        _ => field_value(r, name),
    }
}

/// One displayable field value, built-in or custom, or `None` when unset or empty.
pub(crate) fn field_value(r: &Issue, name: &str) -> Option<String> {
    let some = |s: &str| (!s.is_empty()).then(|| s.to_string());
    match name {
        "id" => some(&r.id),
        "slug" => some(&r.slug),
        "title" => some(&r.title),
        "status" => some(&r.status),
        "priority" => some(&r.priority),
        "points" => Some(r.points.to_string()),
        "parent" => r.parent.clone(),
        "labels" => (!r.labels.is_empty()).then(|| python_list(&r.labels)),
        "depends_on" => (!r.depends_on.is_empty()).then(|| python_list(&r.depends_on)),
        "spec" => r.spec.clone(),
        "review_url" => r.review_url.clone(),
        "created" => r.created.clone(),
        "started" => r.started.clone(),
        "closed" => r.closed.clone(),
        "resolution" => r.resolution.clone(),
        "manual_status" => r.manual_status.then(|| "True".to_string()),
        other => r.extra.get(other).and_then(|v| match v {
            crate::json::Json::String(s) => some(s),
            crate::json::Json::Null => None,
            v => Some(v.to_json()),
        }),
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
    fn unique_prefixes_are_the_fewest_characters_youd_type() {
        let lens = unique_prefix_lens(["aaaaaaa", "aabbbbb", "zzzzzzz"]);
        assert_eq!(lens["aaaaaaa"], 3); // shares "aa" with aabbbbb
        assert_eq!(lens["aabbbbb"], 3);
        assert_eq!(lens["zzzzzzz"], 1);
    }

    #[test]
    fn an_id_that_is_a_prefix_of_another_needs_all_of_itself() {
        let lens = unique_prefix_lens(["ab", "abcd"]);
        assert_eq!(lens["ab"], 2);
    }

    #[test]
    fn a_lone_id_needs_one_character() {
        assert_eq!(unique_prefix_lens(["k3m9x2a"])["k3m9x2a"], 1);
    }
}

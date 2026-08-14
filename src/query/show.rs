//! `show`: one issue's metadata above its hand-authored body.
//!
//! In its own file beside `list` and the path verbs for the same reason they are: the module
//! root should say what the read side *is*, not implement one third of it.

use crate::discovery::Ctx;
use crate::graph::Graph;
use crate::issue::{CANON_KEYS, Issue};
use crate::json::Json;
use crate::render::{field_value_raw, hl_id, paint, unique_prefix_lens};
use crate::verbs::{load_rows, resolve_ref};

/// `show --json`: one document with the body folded in.
///
/// Metadata *and* body together, rather than the human view's metadata-then-separator: the
/// obvious way to consume this is `json.loads(stdout)`, and a trailing `--- body ---` block
/// would break it. `points` is dropped on a parent for the same reason the human view drops
/// it — there it is derived, not an input.
pub(crate) fn cmd_show_json(ctx: &Ctx, token: &str) -> Result<String, String> {
    let (row, body, is_leaf) = show_parts(ctx, token)?;
    let mut obj = match row.to_full() {
        Json::Object(pairs) => pairs,
        _ => Vec::new(),
    };
    if !is_leaf {
        obj.retain(|(k, _)| k != "points");
    }
    obj.push(("body".into(), Json::String(body)));
    Ok(Json::Object(obj).to_json_pretty())
}

/// The row, its body text and whether it is a leaf — everything both `show` renderings need,
/// resolved and guarded once so the two cannot disagree about which issue they are showing.
fn show_parts(ctx: &Ctx, token: &str) -> Result<(Issue, String, bool), String> {
    let rows = load_rows(ctx)?;
    let iid = resolve_ref(&rows, token)?;
    let g = Graph::new(rows);
    let row = g.get(&iid).ok_or_else(|| format!("no issue matching '{iid}'"))?.clone();
    let body = ctx.read_body(&row)?;
    Ok((row, body, g.is_leaf(&iid)))
}

pub(crate) fn cmd_show(ctx: &Ctx, token: &str) -> Result<String, String> {
    let rows = load_rows(ctx)?;
    let iid = resolve_ref(&rows, token)?;
    let abbrev = unique_prefix_lens(rows.iter().map(|r| r.id.as_str()));
    let g = Graph::new(rows);
    let row = g.get(&iid).ok_or_else(|| format!("no issue matching '{iid}'"))?;

    let mut keys: Vec<String> = CANON_KEYS.iter().map(|k| (*k).to_string()).collect();
    if !g.is_leaf(&iid) {
        // Points roll up from leaves, so on a parent the stored value is not an input.
        keys.retain(|k| k != "points");
    }
    keys.extend(row.extra.keys().cloned());

    // The column width comes from *every* candidate key, not only the ones with a
    // value. An issue that happens to carry no `manual_status` still aligns with one
    // that does, so two `show` outputs sit in the same column.
    let width = keys.iter().map(|k| k.chars().count()).max().unwrap_or(0);
    let shown: Vec<(String, String)> = keys.iter().filter_map(|k| field_value_raw(row, k).map(|v| (k.clone(), v))).collect();
    let mut out: Vec<String> = Vec::new();
    for (k, v) in &shown {
        let v = match k.as_str() {
            "created" | "started" | "closed" => v.get(..10).unwrap_or(v).to_string(),
            "id" => hl_id(v, Some(&abbrev), false),
            _ => v.clone(),
        };
        out.push(format!("{}  {v}", paint(&format!("{k:>width$}"), &["dim"])));
    }
    out.push(String::new());
    out.push("--- body ---".into());
    out.push(String::new());
    out.push(ctx.read_body(row)?);
    Ok(out.join("\n"))
}

//! The aligned one-line row format `list` and `ready` share.

use super::annotate::{Annotation, block_annotations, demand_annotation};
use super::fields::field_value;
use super::{gutter, hl_id, paint, priority_codes, status_codes};
use crate::graph::Graph;
use crate::issue::Issue;
use std::collections::BTreeMap;

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

/// The status and priority columns are padded to the widest value *in this batch*, so a
/// listing lines up without a fixed width nobody's vocabulary would fit.
struct Widths {
    status: usize,
    priority: usize,
}

impl Widths {
    fn of(rows: &[&Issue]) -> Widths {
        Widths {
            status: rows.iter().map(|r| r.status.chars().count()).max().unwrap_or(0),
            priority: rows.iter().map(|r| r.priority.chars().count()).max().unwrap_or(0),
        }
    }
}

/// Render issues as aligned one-line summaries, shared by `list` and `ready`.
pub(crate) fn render_rows(g: &Graph, rows: &[&Issue], opts: &RowOpts) -> Vec<String> {
    let widths = Widths::of(rows);
    rows.iter().map(|r| render_row(g, r, opts, &widths)).collect()
}

fn render_row(g: &Graph, r: &Issue, opts: &RowOpts, w: &Widths) -> String {
    let pre = opts.prefix.and_then(|p| p.get(&r.id)).cloned().unwrap_or_default();
    let prog = if opts.progress { g.progress_pct(&r.id).map_or_else(String::new, |p| format!(" {p}%")) } else { String::new() };
    // The connector already shows the parent when a row is nested under it.
    let tags = row_tags(g, r, pre.is_empty());
    let ann = annotation(g, r, opts);
    let fsuf = field_suffix(r, &opts.show_fields);
    // The glyph carries readiness on its own; the status word beside it stays the
    // status word, coloured by status like every other row's.
    let (icon, icodes) = gutter(&r.status, g.is_ready(&r.id));
    let (sw, pw) = (w.status, w.priority);

    if opts.dim.contains(&r.id) {
        // Ancestor context: the whole line dims, with no per-field colour.
        let body = format!("{icon} #{} {:<sw$}  {:<pw$}  {pre}{}{prog}{tags}", r.id, r.status, r.priority, r.title);
        return format!("{}{ann}{fsuf}", paint(&body, &["dim"]));
    }
    format!(
        "{} {} {}  {}  {pre}{}{}{}{ann}{fsuf}",
        paint(icon, icodes),
        hl_id(&r.id, opts.abbrev.as_ref(), true),
        paint(&format!("{:<sw$}", r.status), &status_codes(&r.status)),
        paint(&format!("{:<pw$}", r.priority), &priority_codes(&r.priority)),
        r.title,
        paint(&prog, &["dim"]),
        paint(&tags, &["dim"]),
    )
}

/// The bracketed tags: `EPIC`, the parent when the connector is not already showing it, then
/// the labels.
///
/// `EPIC` is derived, not declared: an issue with children *is* an epic. As a stored kind the
/// two drifted, and nothing ever stopped them.
fn row_tags(g: &Graph, r: &Issue, show_parent: bool) -> String {
    let mut tags: Vec<String> = Vec::new();
    if !g.children_of(&r.id).is_empty() {
        tags.push("EPIC".into());
    }
    if let Some(parent) = &r.parent
        && show_parent
    {
        tags.push(format!("↳{parent}"));
    }
    tags.extend(r.labels.iter().cloned());
    if tags.is_empty() { String::new() } else { format!(" [{}]", tags.join(" ")) }
}

/// Whichever trailing note this view asked for.
fn annotation(g: &Graph, r: &Issue, opts: &RowOpts) -> String {
    match opts.annotate {
        Annotation::None => String::new(),
        Annotation::Blocking => block_annotations(g, &r.id, &opts.on_screen),
        Annotation::Demand => demand_annotation(g, &r.id, opts.abbrev.as_ref()),
    }
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

#[cfg(test)]
mod tests {
    // Tests assert; that is their job. The crate denies unwrap/expect/panic because a
    // malformed tracker must produce a diagnostic rather than a stack trace, but a test
    // that cannot panic cannot fail.
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;
    use crate::test_graph::graph;

    fn opts<'a>() -> RowOpts<'a> {
        RowOpts { prefix: None, dim: &[], on_screen: Vec::new(), annotate: Annotation::None, progress: false, show_fields: Vec::new(), abbrev: None }
    }

    fn render(spec: &[&str], o: &RowOpts) -> Vec<String> {
        let g = graph(spec);
        let rows: Vec<&Issue> = g.rows.iter().collect();
        render_rows(&g, &rows, o)
    }

    /// Character index, not byte index: the status glyph and the tag arrow are multi-byte, so
    /// a byte offset would compare two different things.
    fn col_of(line: &str, needle: &str) -> usize {
        let at = line.find(needle).unwrap_or_else(|| panic!("{needle:?} not in {line:?}"));
        line[..at].chars().count()
    }

    /// The status and priority columns are padded to the widest value in the batch, so the
    /// column after each one starts at the same place on every row — which is the whole reason
    /// the widths are computed over the batch rather than fixed.
    #[test]
    fn the_columns_align_to_the_widest_value_present() {
        // Ids chosen not to collide with any status or priority word.
        let out = render(&["zzz @backlog !high", "yyy @in-progress !lowest"], &opts());
        assert_eq!(out.len(), 2);
        assert_eq!(col_of(&out[0], "high"), col_of(&out[1], "lowest"), "priority column moved:\n{}\n{}", out[0], out[1]);
        assert_eq!(col_of(&out[0], "zzz "), col_of(&out[1], "yyy "), "title column moved:\n{}\n{}", out[0], out[1]);
    }

    /// A one-row batch pads to that row's own width, so a single listing has no trailing run
    /// of spaces before the title.
    #[test]
    fn a_single_row_is_padded_to_itself() {
        // Two spaces is the column separator; a third would mean it padded to something wider
        // than the only row present.
        let out = render(&["zzz @backlog !high"], &opts());
        assert!(!out[0].contains("backlog   "), "over-padded: {:?}", out[0]);
        assert_eq!(out[0], "◇ #zzz backlog  high  zzz");
    }

    /// A parent with children is tagged `EPIC` whether or not anyone stored a kind.
    #[test]
    fn a_row_with_children_is_tagged_an_epic() {
        let out = render(&["epic", "kid:epic"], &opts());
        assert!(out.iter().any(|l| l.contains("[EPIC]")), "{out:?}");
    }

    /// The parent tag appears only when no connector prefix is showing it already.
    #[test]
    fn the_parent_tag_yields_to_the_connector() {
        let g = graph(&["epic", "kid:epic"]);
        let rows: Vec<&Issue> = g.rows.iter().filter(|r| r.id == "kid").collect();
        let bare = render_rows(&g, &rows, &opts());
        assert!(bare[0].contains("↳epic"), "{bare:?}");

        let mut prefix = BTreeMap::new();
        prefix.insert("kid".to_string(), "└─ ".to_string());
        let nested = render_rows(&g, &rows, &RowOpts { prefix: Some(&prefix), ..opts() });
        assert!(!nested[0].contains("↳epic"), "the connector already says it: {nested:?}");
    }

    #[test]
    fn labels_ride_in_the_same_brackets_as_the_epic_tag() {
        let out = render(&["a +one +two"], &opts());
        assert!(out[0].contains("[one two]"), "{out:?}");
    }

    /// A dimmed row is one string with no per-field colour, and still carries its note and
    /// its field columns.
    #[test]
    fn a_dimmed_row_keeps_its_note_and_columns() {
        let g = graph(&["a"]);
        let rows: Vec<&Issue> = g.rows.iter().collect();
        let dim = vec!["a".to_string()];
        let out = render_rows(&g, &rows, &RowOpts { dim: &dim, show_fields: vec!["points".into()], ..opts() });
        assert!(out[0].contains("points="), "{out:?}");
    }

    #[test]
    fn show_field_columns_appear_only_for_fields_that_have_a_value() {
        let g = graph(&["a"]);
        let rows: Vec<&Issue> = g.rows.iter().collect();
        let out = render_rows(&g, &rows, &RowOpts { show_fields: vec!["points".into(), "spec".into()], ..opts() });
        assert!(out[0].contains("points="), "{out:?}");
        assert!(!out[0].contains("spec="), "an unset field contributes no column: {out:?}");
    }

    #[test]
    fn no_show_fields_means_no_trailing_columns() {
        let out = render(&["a"], &opts());
        assert!(!out[0].contains('='), "{out:?}");
    }

    /// An empty batch must not panic on the column widths — `list` with everything filtered
    /// out reaches here.
    #[test]
    fn an_empty_batch_renders_nothing_rather_than_panicking() {
        let g = graph(&["a"]);
        assert!(render_rows(&g, &[], &opts()).is_empty());
    }
}

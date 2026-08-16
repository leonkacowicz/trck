//! What `list` keeps.
//!
//! Split from [`super::list`], which decides how the kept rows are *shown*. Those are
//! different questions, and the filtering half is most of the weight: eleven conditions,
//! all the same shape, plus the one that has to go and look at something.
//!
//! That one is `--contains`. Everything else here is a question about a row the index
//! already answered, and [`RowFilter::keeps`] stays that way — pure, no `Ctx`, no I/O — by
//! having [`body_hits`] run the search **once** before any row is tested and handing the
//! result down as a set of ids.

use super::ListOpts;
use crate::config::is_terminal;
use crate::discovery::Ctx;
use crate::graph::Graph;
use crate::issue::{Issue, check_field_key};
use crate::render::field_value_raw;
use std::collections::BTreeSet;

/// `--status a,b` keeps those; `--status '!done'` drops them. Returns `(keep, drop)`.
///
/// Names are canonicalised, so a filter spelled with a retired status selects the rows
/// that status became — which is the only reading that can match anything, since no row
/// is stored under the old name.
fn parse_status_filter(spec: Option<&str>) -> (BTreeSet<String>, BTreeSet<String>) {
    let (mut keep, mut drop) = (BTreeSet::new(), BTreeSet::new());
    for part in spec.unwrap_or("").split(',').map(str::trim) {
        if part.is_empty() {
            continue;
        }
        if let Some(name) = part.strip_prefix('!') {
            drop.insert(crate::config::canonical_status(name).to_string());
        } else {
            keep.insert(crate::config::canonical_status(part).to_string());
        }
    }
    (keep, drop)
}

/// Validate the option combination before any work: an unknown `--sort` or a malformed
/// `--field` should be reported as such, not silently ignored.
fn check_list_opts(opts: &ListOpts) -> Result<Vec<(String, String)>, String> {
    // Two output modes asked for at once. Here rather than beside the rendering it refuses,
    // which is where it used to sit: this function is already "everything wrong with the
    // combination", and asked here the refusal comes before the tracker is read rather than
    // after a forest has been selected and sorted for nobody.
    if opts.paths && opts.json {
        return Err("--paths and --json are different output modes; pick one".into());
    }
    let mut field_filters = Vec::new();
    for spec in &opts.fields {
        let (k, v) = spec.split_once('=').ok_or_else(|| format!("--field expects key=value, got '{spec}'"))?;
        // The same key rule the write side enforces. Without it a filter on a built-in —
        // `--field status=backlog` — would look up a custom field that can never exist and
        // quietly match nothing, or worse, appear to work.
        if let Some(msg) = check_field_key(k) {
            return Err(msg);
        }
        field_filters.push((k.to_string(), v.to_string()));
    }
    if let Some(s) = opts.sort
        && !["priority", "points", "created", "id"].contains(&s)
        && !s.starts_with("field:")
    {
        return Err(format!("unknown --sort '{s}' (choices: id, priority, points, created, field:NAME)"));
    }
    Ok(field_filters)
}

/// Settled work: terminal, under a parent that is also terminal (or none at all).
///
/// The default view hides it, which is what keeps `trck list` about what is left rather
/// than about everything that ever happened.
fn is_settled(g: &Graph, r: &Issue) -> bool {
    is_terminal(&r.status) && r.parent.as_ref().and_then(|p| g.get(p)).is_none_or(|p| is_terminal(&p.status))
}

/// The ids whose body holds `--contains`, resolved once for the whole invocation.
///
/// This is `list`'s only reason to read anything but the index, and it reads everything in
/// one search rather than a body per row — see [`Ctx::body_matches`] for why that
/// distinction is the design and not an optimisation.
///
/// An empty pattern filters nothing rather than matching everything the long way round.
/// It is the reading `--match ''` already gets, it is the same answer, and it saves asking
/// git a question whose answer is "all of them".
///
/// A body file git returned that no row claims is dropped in silence, because the mapping
/// runs the other way: a row's file is whatever [`crate::summary::filename`] says it is, so
/// a name outside that set belongs to no issue this tracker can show.
pub(super) fn body_hits(ctx: &Ctx, rows: &[Issue], needle: Option<&str>) -> Result<Option<BTreeSet<String>>, String> {
    let Some(needle) = needle.filter(|n| !n.is_empty()) else {
        return Ok(None);
    };
    let files = ctx.body_matches(needle)?;
    Ok(Some(rows.iter().filter(|r| files.contains(&crate::summary::filename(r))).map(|r| r.id.clone()).collect()))
}

/// Everything `list` selects on, resolved once from the options and the graph.
///
/// Gathered into a type rather than left as a closure with a dozen captures: the conditions
/// are the bulk of what `cmd_list` used to be, they are all the same shape, and none of them
/// has anything to do with choosing an output format — which is all that is left there now.
pub(super) struct RowFilter<'a> {
    keep_status: BTreeSet<String>,
    drop_status: BTreeSet<String>,
    priority: Option<&'a str>,
    label: Option<&'a str>,
    parent: Option<String>,
    title_match: String,
    /// The ids `--contains` matched, or `None` when it was not asked for. Already a set by
    /// the time it gets here; see [`body_hits`].
    contains: Option<BTreeSet<String>>,
    fields: Vec<(String, String)>,
    blocked_only: bool,
    orphans_only: bool,
    hide_settled: bool,
}

impl RowFilter<'_> {
    pub(super) fn build<'b>(opts: &'b ListOpts, parent: Option<String>, contains: Option<BTreeSet<String>>) -> Result<RowFilter<'b>, String> {
        let (keep_status, drop_status) = parse_status_filter(opts.status);
        Ok(RowFilter {
            keep_status,
            drop_status,
            priority: opts.priority,
            label: opts.label,
            parent,
            title_match: opts.match_title.unwrap_or("").to_lowercase(),
            contains,
            fields: check_list_opts(opts)?,
            blocked_only: opts.blocked,
            orphans_only: opts.orphan,
            // An explicit --status or --all bypasses the default hiding.
            hide_settled: opts.status.is_none() && !opts.all,
        })
    }

    /// Every filter, AND-ed. Split in two by what each half needs to look at rather than
    /// left as one run of conditions: the list only ever grows, and two names beat a
    /// twelve-line boolean nobody can find the middle of.
    pub(super) fn keeps(&self, g: &Graph, r: &Issue) -> bool {
        self.row_keeps(r) && self.shape_keeps(g, r)
    }

    /// The conditions a row answers by itself.
    ///
    /// `--contains` is here, and it is a set lookup, because the search that built the set
    /// ran once before any of this. That is the whole point of the pre-pass: this function
    /// takes no `Ctx` and does no I/O, and every row-shaped filter can stay cheap.
    fn row_keeps(&self, r: &Issue) -> bool {
        (self.keep_status.is_empty() || self.keep_status.contains(&r.status))
            && !self.drop_status.contains(&r.status)
            && self.priority.is_none_or(|p| r.priority == p)
            && self.label.is_none_or(|l| r.labels.iter().any(|x| x == l))
            && (self.title_match.is_empty() || r.title.to_lowercase().contains(&self.title_match))
            && self.contains.as_ref().is_none_or(|ids| ids.contains(&r.id))
            && self.fields.iter().all(|(k, v)| field_value_raw(r, k).as_ref() == Some(v))
    }

    /// The conditions about where a row sits rather than what it says.
    fn shape_keeps(&self, g: &Graph, r: &Issue) -> bool {
        self.parent.as_ref().is_none_or(|p| r.parent.as_ref() == Some(p))
            && (!self.blocked_only || g.is_blocked(&r.id))
            && (!self.orphans_only || r.parent.is_none())
            && (!self.hide_settled || !is_settled(g, r))
    }
}

#[cfg(test)]
mod tests {
    // Tests assert; that is their job. The crate denies unwrap/expect/panic because a
    // malformed tracker must produce a diagnostic rather than a stack trace, but a test
    // that cannot panic cannot fail.
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;
    use crate::discovery::Source;
    use crate::discovery::tests::Tmp;

    #[test]
    fn a_status_filter_separates_keeps_from_drops() {
        let (keep, drop) = parse_status_filter(Some("backlog,in-progress"));
        assert_eq!(keep.len(), 2);
        assert!(drop.is_empty());
        let (keep, drop) = parse_status_filter(Some("!done"));
        assert!(keep.is_empty());
        assert!(drop.contains("done"));
    }

    #[test]
    fn a_retired_status_name_filters_on_what_it_became() {
        // No row is stored under the old name, so a filter that kept it verbatim could
        // only ever match nothing — on either side of the negation.
        let (keep, drop) = parse_status_filter(Some("ongoing"));
        assert!(keep.contains("in-progress"));
        assert!(!keep.contains("ongoing"));
        let (_, drop_legacy) = parse_status_filter(Some("!ongoing"));
        assert!(drop_legacy.contains("in-progress"));
        assert!(drop.is_empty());
    }

    #[test]
    fn an_absent_filter_keeps_everything() {
        let (keep, drop) = parse_status_filter(None);
        assert!(keep.is_empty() && drop.is_empty());
    }

    /// A tracker with two issues, one of whose bodies holds `race condition`.
    fn seeded(tag: &str) -> (Tmp, Ctx, Vec<Issue>) {
        let tmp = Tmp::new(tag);
        let d = tmp.tracker("issues");
        std::fs::create_dir_all(d.join("items")).expect("mkdir");
        std::fs::write(d.join("items/aaa1111-a.md"), "# a\n\nA Race Condition here.\n").expect("write");
        std::fs::write(d.join("items/bbb2222-b.md"), "# b\n\nnothing.\n").expect("write");
        let index = "{\"id\": \"aaa1111\", \"slug\": \"a\", \"title\": \"a\", \"status\": \"backlog\", \"priority\": \"medium\"}\n\
                     {\"id\": \"bbb2222\", \"slug\": \"b\", \"title\": \"b\", \"status\": \"backlog\", \"priority\": \"medium\"}\n";
        let rows = crate::index::parse_index(index, "index.jsonl").expect("parses");
        let ctx = Ctx::load(Source::Dir(d), false).expect("loads");
        (tmp, ctx, rows)
    }

    #[test]
    fn body_hits_answers_with_the_ids_whose_body_matches() {
        let (_tmp, ctx, rows) = seeded("bodyhits");
        let hits = body_hits(&ctx, &rows, Some("race condition")).expect("searched").expect("a filter");
        assert_eq!(hits, ["aaa1111".to_string()].into_iter().collect());
    }

    /// Nothing found is an empty filter, not an absent one: the difference is `list
    /// --contains xyzzy` printing nothing versus printing the whole tracker.
    #[test]
    fn a_pattern_that_matches_nothing_is_an_empty_set_rather_than_no_filter() {
        let (_tmp, ctx, rows) = seeded("bodyhits-none");
        let hits = body_hits(&ctx, &rows, Some("nothing holds this")).expect("searched");
        assert_eq!(hits, Some(BTreeSet::new()));
    }

    /// No flag and an empty one both mean "do not filter on the body" — and neither asks
    /// git anything, which is what makes `list` on a tracker with no git still work.
    #[test]
    fn an_absent_or_empty_pattern_is_no_filter_at_all() {
        let (_tmp, ctx, rows) = seeded("bodyhits-empty");
        assert_eq!(body_hits(&ctx, &rows, None).expect("searched"), None);
        assert_eq!(body_hits(&ctx, &rows, Some("")).expect("searched"), None);
    }
}

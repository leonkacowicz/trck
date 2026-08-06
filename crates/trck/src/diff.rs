//! `diff` — what changed between two tracker states — and `changelog`.
//!
//! The design worth knowing is the **source seam**: `diff` compares two [`Snapshot`]s and
//! has no notion of a revision. Whoever produced the snapshots owns that, which is what
//! lets `--from FILE`, `--from -`, a git revision and the working tree all feed the same
//! comparison. Git is a layer on top, not a dependency of the model.
//!
//! The change model is deliberately three-way. A scalar field that moved, a set field
//! that gained and lost members, and a timestamp are different questions, and flattening
//! them into "these keys differ" would lose the one thing a reader wants — whether a
//! `done -> ongoing` move is a reopen or a start.

use crate::config::{self, is_terminal};
use crate::discovery::Ctx;
use crate::graph::Graph;
use crate::index::parse_index;
use crate::issue::{CANON_KEYS, Issue};
use crate::render::field_value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

const TIMESTAMP_FIELDS: &[&str] = &["created", "started", "closed"];
const SET_FIELDS: &[&str] = &["labels", "depends_on"];

/// Everything comparable as a scalar: the canonical fields minus the id, the timestamps
/// and the set-valued ones.
fn scalar_fields() -> Vec<&'static str> {
    CANON_KEYS.iter().copied().filter(|k| *k != "id" && !TIMESTAMP_FIELDS.contains(k) && !SET_FIELDS.contains(k)).collect()
}

/// A tracker state, and a label naming where it came from.
pub(crate) struct Snapshot {
    pub(crate) label: String,
    pub(crate) rows: Vec<Issue>,
}

impl Snapshot {
    fn from_text(text: &str, label: &str) -> Result<Snapshot, String> {
        Ok(Snapshot { label: label.to_string(), rows: parse_index(text, label)? })
    }
}

/// A scalar field that differs between the two sides.
pub(crate) struct FieldDelta {
    pub(crate) name: String,
    pub(crate) old: Option<String>,
    pub(crate) new: Option<String>,
}

/// A set-valued field with what it gained and lost.
pub(crate) struct SetDelta {
    pub(crate) name: String,
    pub(crate) added: Vec<String>,
    pub(crate) removed: Vec<String>,
}

/// What happened to one issue between the two snapshots.
pub(crate) struct Change {
    pub(crate) id: String,
    pub(crate) kind: &'static str,
    pub(crate) title: String,
    pub(crate) fields: Vec<FieldDelta>,
    pub(crate) sets: Vec<SetDelta>,
    pub(crate) timestamps: BTreeMap<String, (Option<String>, Option<String>)>,
    /// `forward`, `backward` or `lateral`; `None` when the status did not move.
    pub(crate) direction: Option<&'static str>,
}

/// Classify a status move against the vocabulary order.
///
/// A status the engine does not know — an old snapshot written under a renamed
/// vocabulary — is unordered, so the move is `lateral` rather than an error. A renderer
/// needs this because a `done -> ongoing` reopen must not read like a `backlog ->
/// ongoing` start.
pub(crate) fn status_direction(old: &str, new: &str) -> Option<&'static str> {
    if old == new {
        return None;
    }
    let pos = |s: &str| config::STATUSES.iter().position(|x| *x == s);
    match (pos(old), pos(new)) {
        (Some(a), Some(b)) if b > a => Some("forward"),
        (Some(_), Some(_)) => Some("backward"),
        _ => Some("lateral"),
    }
}

/// Every comparable scalar of a row, built-in and custom alike.
fn values(row: &Issue) -> BTreeMap<String, String> {
    let mut out: BTreeMap<String, String> = scalar_fields().into_iter().filter_map(|k| field_value(row, k).map(|v| (k.to_string(), v))).collect();
    for (k, v) in &row.extra {
        if let Some(s) = v.as_str() {
            out.insert(k.clone(), s.to_string());
        } else {
            out.insert(k.clone(), v.to_json());
        }
    }
    out
}

/// One issue present on both sides. `None` when nothing moved.
fn compare(old: &Issue, new: &Issue) -> Option<Change> {
    let (ov, nv) = (values(old), values(new));
    let keys: BTreeSet<&String> = ov.keys().chain(nv.keys()).collect();
    let fields: Vec<FieldDelta> = keys
        .into_iter()
        .filter(|k| ov.get(*k) != nv.get(*k))
        .map(|k| FieldDelta { name: k.clone(), old: ov.get(k).cloned(), new: nv.get(k).cloned() })
        .collect();

    let mut sets = Vec::new();
    for name in SET_FIELDS {
        let pick =
            |r: &Issue| -> BTreeSet<String> { if *name == "labels" { r.labels.iter().cloned().collect() } else { r.depends_on.iter().cloned().collect() } };
        let (a, b) = (pick(old), pick(new));
        if a != b {
            sets.push(SetDelta { name: (*name).to_string(), added: b.difference(&a).cloned().collect(), removed: a.difference(&b).cloned().collect() });
        }
    }

    let stamp = |r: &Issue, name: &str| match name {
        "created" => r.created.clone(),
        "started" => r.started.clone(),
        _ => r.closed.clone(),
    };
    let timestamps: BTreeMap<String, (Option<String>, Option<String>)> =
        TIMESTAMP_FIELDS.iter().filter(|k| stamp(old, k) != stamp(new, k)).map(|k| ((*k).to_string(), (stamp(old, k), stamp(new, k)))).collect();

    if fields.is_empty() && sets.is_empty() && timestamps.is_empty() {
        return None;
    }
    Some(Change {
        id: new.id.clone(),
        kind: "modified",
        title: new.title.clone(),
        fields,
        sets,
        timestamps,
        direction: status_direction(&old.status, &new.status),
    })
}

/// Join two snapshots by id and classify what changed. Pure: no I/O, and no notion of a
/// revision — whoever produced the snapshots owns that.
pub(crate) fn diff_snapshots(old: &Snapshot, new: &Snapshot) -> Vec<Change> {
    let olds: BTreeMap<&str, &Issue> = old.rows.iter().map(|r| (r.id.as_str(), r)).collect();
    let news: BTreeMap<&str, &Issue> = new.rows.iter().map(|r| (r.id.as_str(), r)).collect();
    let ids: BTreeSet<&str> = olds.keys().chain(news.keys()).copied().collect();
    let mut changes = Vec::new();
    for id in ids {
        match (olds.get(id), news.get(id)) {
            (None, Some(n)) => changes.push(Change {
                id: id.to_string(),
                kind: "added",
                title: n.title.clone(),
                fields: Vec::new(),
                sets: Vec::new(),
                timestamps: BTreeMap::new(),
                direction: None,
            }),
            (Some(o), None) => changes.push(Change {
                id: id.to_string(),
                kind: "removed",
                title: o.title.clone(),
                fields: Vec::new(),
                sets: Vec::new(),
                timestamps: BTreeMap::new(),
                direction: None,
            }),
            (Some(o), Some(n)) => {
                if let Some(c) = compare(o, n) {
                    changes.push(c);
                }
            },
            (None, None) => {},
        }
    }
    changes
}

/// A compact, plain-text account of what moved on one issue.
pub(crate) fn change_summary(c: &Change) -> String {
    let show = |v: &Option<String>| v.clone().unwrap_or_else(|| "None".to_string());
    let mut bits: Vec<String> = c.fields.iter().map(|f| format!("{} {} → {}", f.name, show(&f.old), show(&f.new))).collect();
    for s in &c.sets {
        let members: Vec<String> = s.added.iter().map(|v| format!("+{v}")).chain(s.removed.iter().map(|v| format!("-{v}"))).collect();
        bits.push(format!("{} {}", s.name, members.join(" ")));
    }
    if bits.is_empty() {
        // A timestamp-only edit still changed something; say what.
        bits = c.timestamps.iter().map(|(k, (a, b))| format!("{k} {} → {}", show(a), show(b))).collect();
    }
    bits.join(", ")
}

// --------------------------------------------------------------------------- //
// sources
// --------------------------------------------------------------------------- //

const USE_FROM: &str = "use --from/--to with file paths instead";

fn git_run(args: &[&str], cwd: &Path) -> Result<std::process::Output, String> {
    std::process::Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .map_err(|_| format!("git is not on PATH, so revision specs are unavailable; {USE_FROM}"))
}

/// The tracker dir as a repo-relative prefix, the way `git show <rev>:<path>` wants it.
/// A tracker dir that *is* the repo root yields an empty prefix.
fn git_tracker_prefix(ctx: &Ctx) -> Result<String, String> {
    let out = git_run(&["rev-parse", "--show-toplevel"], &ctx.dir)?;
    if !out.status.success() {
        return Err(format!("not a git repository, so revision specs are unavailable; {USE_FROM}"));
    }
    let root = Path::new(String::from_utf8_lossy(&out.stdout).trim()).to_path_buf();
    let dir = ctx.dir.canonicalize().unwrap_or_else(|_| ctx.dir.clone());
    let root = root.canonicalize().unwrap_or(root);
    let rel = dir.strip_prefix(&root).map_err(|_| format!("tracker dir {} is not inside the git repo at {}", ctx.dir.display(), root.display()))?;
    let rel = rel.to_string_lossy().replace('\\', "/");
    Ok(if rel.is_empty() || rel == "." { String::new() } else { format!("{rel}/") })
}

/// The tracker as of `rev`.
///
/// A tracker dir absent at that revision is **not** an error: comparing against a commit
/// from before the tracker existed is a legitimate question, and the answer is "every
/// issue is new". An unresolvable revision *is* an error, reported separately, so "you
/// typo'd the branch" stays distinguishable from "the tracker did not exist yet".
pub(crate) fn git_snapshot(ctx: &Ctx, rev: &str) -> Result<Snapshot, String> {
    let verify = git_run(&["rev-parse", "--verify", "--quiet", &format!("{rev}^{{commit}}")], &ctx.dir)?;
    if !verify.status.success() {
        return Err(format!("unknown revision '{rev}'"));
    }
    let prefix = git_tracker_prefix(ctx)?;
    let path = format!("{prefix}index.jsonl");
    let out = git_run(&["show", &format!("{rev}:{path}")], &ctx.dir)?;
    let text = if out.status.success() { String::from_utf8_lossy(&out.stdout).into_owned() } else { String::new() };
    Snapshot::from_text(&text, rev)
}

/// Split a revision spec into `(old, new)`; a `None` new side means the working tree.
pub(crate) fn parse_rev_spec(spec: &str) -> Result<(String, Option<String>), String> {
    if spec.contains("...") {
        return Err("three-dot (merge-base) revision specs are not supported; \
                    use `a..b` to compare two revisions directly"
            .to_string());
    }
    let Some((old, new)) = spec.split_once("..") else {
        return Ok((spec.to_string(), None));
    };
    if old.is_empty() || new.is_empty() {
        return Err(format!("incomplete revision range '{spec}'; both sides of `..` are required"));
    }
    Ok((old.to_string(), Some(new.to_string())))
}

/// Resolve a `--from`/`--to` spec: a file, a directory holding one, `-` for stdin, or the
/// working tree when unspecified.
pub(crate) fn resolve_source(spec: Option<&str>, ctx: &Ctx) -> Result<Snapshot, String> {
    let Some(spec) = spec else {
        let text = std::fs::read_to_string(ctx.index_path()).unwrap_or_default();
        return Snapshot::from_text(&text, "working tree");
    };
    if spec == "-" {
        use std::io::Read as _;
        let mut text = String::new();
        std::io::stdin().read_to_string(&mut text).map_err(|e| format!("stdin: {e}"))?;
        return Snapshot::from_text(&text, "stdin");
    }
    let path = Path::new(spec);
    // The label is the file's own name, not the spec that named it: a long relative path
    // buries the one word that identifies the side being compared.
    let label = path.file_name().map_or_else(|| spec.to_string(), |n| n.to_string_lossy().into_owned());
    if path.is_dir() {
        // A tracker dir with no index is an empty snapshot, not an error: the tracker
        // not existing on one side is a legitimate comparison, and everything on the
        // other side reads as added.
        let text = std::fs::read_to_string(path.join("index.jsonl")).unwrap_or_default();
        return Snapshot::from_text(&text, &label);
    }
    let text = std::fs::read_to_string(path).map_err(|_| format!("no such file: {spec}"))?;
    Snapshot::from_text(&text, &label)
}

// --------------------------------------------------------------------------- //
// changelog
// --------------------------------------------------------------------------- //

/// Validate a `--since` cutoff: a bare date or a full UTC timestamp.
pub(crate) fn parse_since(value: &str) -> Result<String, String> {
    let date_ok = value.len() >= 10 && value.as_bytes()[..10].iter().enumerate().all(|(i, b)| if i == 4 || i == 7 { *b == b'-' } else { b.is_ascii_digit() });
    let ok = date_ok
        && (value.len() == 10
            || (value.len() == 20
                && value.as_bytes()[10] == b'T'
                && value.ends_with('Z')
                && value[11..19].bytes().enumerate().all(|(i, b)| if i == 2 || i == 5 { b == b':' } else { b.is_ascii_digit() })));
    if ok { Ok(value.to_string()) } else { Err(format!("--since must be a date (YYYY-MM-DD) or timestamp (YYYY-MM-DDTHH:MM:SSZ), got '{value}'")) }
}

/// Issues that *shipped* on or after `since`.
///
/// Terminal, closed in the window, and carrying **no resolution** — so wontfix, duplicate
/// and superseded are excluded. That absence is the whole rule: `done` alone does not
/// mean anything shipped, and an issue closed without producing something has no release
/// note to write.
pub(crate) fn select_shipped(rows: &[Issue], since: &str) -> Vec<Issue> {
    rows.iter()
        .filter(|r| is_terminal(&r.status))
        .filter(|r| r.closed.as_deref().is_some_and(|c| c >= since))
        .filter(|r| r.resolution.is_none())
        .cloned()
        .collect()
}

fn walk(g: &Graph, out: &mut Vec<String>, id: &str, depth: usize, seen: &BTreeSet<String>) {
    let Some(node) = g.get(id) else { return };
    let tag = node.extra.get("component").and_then(|v| v.as_str()).map_or_else(String::new, |c| format!(" ({c})"));
    out.push(format!("{}- #{} {}{tag}", "  ".repeat(depth), node.id, node.title));
    if seen.contains(id) {
        return;
    }
    let mut kids: Vec<String> = g.children_of(id).to_vec();
    kids.sort();
    kids.sort_by(|a, b| {
        let closed = |i: &String| g.get(i).and_then(|r| r.closed.clone()).unwrap_or_default();
        closed(b).cmp(&closed(a))
    });
    let mut next = seen.clone();
    next.insert(id.to_string());
    for kid in kids {
        walk(g, out, &kid, depth + 1, &next);
    }
}

/// Render the changelog: shipped issues nested under shipped parents, newest first.
pub(crate) fn render_changelog(shipped: &[Issue], since: &str) -> String {
    let n = shipped.len();
    let header = format!("## Shipped since {since} — {n} issue{}", if n == 1 { "" } else { "s" });
    if shipped.is_empty() {
        return format!("{header}\n\n_none_\n");
    }
    let g = Graph::new(shipped.to_vec());
    let mut out = vec![header, String::new()];

    // Closed descending, id ascending on ties — a stable sort over an id-sorted input.
    let sib_sorted = |mut ids: Vec<String>| -> Vec<String> {
        ids.sort();
        ids.sort_by(|a, b| {
            let closed = |i: &String| g.get(i).and_then(|r| r.closed.clone()).unwrap_or_default();
            closed(b).cmp(&closed(a))
        });
        ids
    };

    let roots: Vec<String> = g.rows.iter().filter(|r| r.parent.as_ref().is_none_or(|p| g.get(p).is_none())).map(|r| r.id.clone()).collect();
    for root in sib_sorted(roots) {
        walk(&g, &mut out, &root, 0, &BTreeSet::new());
    }
    out.push(String::new());
    out.join("\n")
}

#[cfg(test)]
mod tests {
    // Tests assert; that is their job. The crate denies unwrap/expect/panic because a
    // malformed tracker must produce a diagnostic rather than a stack trace, but a test
    // that cannot panic cannot fail.
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;
    use crate::json::parse;

    fn issue(json: &str) -> Issue {
        Issue::from_json(&parse(json).expect("json")).expect("issue")
    }

    const BASE: &str = r#"{"id": "aaaaaaa", "slug": "a", "title": "A", "status": "backlog", "priority": "medium"}"#;

    #[test]
    fn a_reopen_reads_differently_from_a_start() {
        // The reason `direction` exists: both are status moves, and a renderer that
        // cannot tell them apart reports a regression as progress.
        assert_eq!(status_direction("backlog", "ongoing"), Some("forward"));
        assert_eq!(status_direction("done", "ongoing"), Some("backward"));
        assert_eq!(status_direction("done", "done"), None);
    }

    #[test]
    fn a_status_outside_the_vocabulary_is_lateral_not_an_error() {
        // An old snapshot written under a renamed vocabulary is unordered, not invalid.
        assert_eq!(status_direction("shipped", "done"), Some("lateral"));
    }

    #[test]
    fn an_unchanged_issue_produces_no_change() {
        let a = issue(BASE);
        assert!(compare(&a, &a).is_none());
    }

    #[test]
    fn a_scalar_move_is_reported_with_both_sides() {
        let a = issue(BASE);
        let b = issue(&BASE.replace(r#""priority": "medium""#, r#""priority": "urgent""#));
        let c = compare(&a, &b).expect("changed");
        assert_eq!(c.fields.len(), 1);
        assert_eq!(c.fields[0].name, "priority");
        assert_eq!(c.fields[0].old.as_deref(), Some("medium"));
        assert_eq!(c.fields[0].new.as_deref(), Some("urgent"));
    }

    #[test]
    fn a_set_field_reports_what_it_gained_and_lost() {
        let a = issue(&BASE.replace(r#""priority": "medium""#, r#""priority": "medium", "labels": ["x", "y"]"#));
        let b = issue(&BASE.replace(r#""priority": "medium""#, r#""priority": "medium", "labels": ["y", "z"]"#));
        let c = compare(&a, &b).expect("changed");
        assert_eq!(c.sets.len(), 1);
        assert_eq!(c.sets[0].added, ["z"]);
        assert_eq!(c.sets[0].removed, ["x"]);
    }

    #[test]
    fn a_timestamp_only_edit_still_says_what_moved() {
        let a = issue(BASE);
        let b = issue(&BASE.replace(r#""priority": "medium""#, r#""priority": "medium", "started": "2026-01-01T00:00:00Z""#));
        let c = compare(&a, &b).expect("changed");
        assert!(c.fields.is_empty() && c.sets.is_empty());
        assert!(change_summary(&c).contains("started"));
    }

    #[test]
    fn a_custom_field_is_compared_like_any_other_scalar() {
        let a = issue(&BASE.replace(r#""priority": "medium""#, r#""priority": "medium", "assignee": "alice""#));
        let b = issue(&BASE.replace(r#""priority": "medium""#, r#""priority": "medium", "assignee": "bo""#));
        let c = compare(&a, &b).expect("changed");
        assert_eq!(c.fields[0].name, "assignee");
    }

    #[test]
    fn added_and_removed_are_joined_by_id() {
        let old = Snapshot { label: "old".into(), rows: vec![issue(BASE)] };
        let new = Snapshot { label: "new".into(), rows: vec![issue(&BASE.replace("aaaaaaa", "bbbbbbb"))] };
        let changes = diff_snapshots(&old, &new);
        // Joined by id, and reported in id order: aaaaaaa (gone) before bbbbbbb (new).
        let seen: Vec<(&str, &str)> = changes.iter().map(|c| (c.id.as_str(), c.kind)).collect();
        assert_eq!(seen, [("aaaaaaa", "removed"), ("bbbbbbb", "added")]);
    }

    #[test]
    fn a_revision_range_names_both_sides() {
        assert_eq!(parse_rev_spec("HEAD").expect("ok"), ("HEAD".into(), None));
        assert_eq!(parse_rev_spec("a..b").expect("ok"), ("a".to_string(), Some("b".to_string())));
        assert!(parse_rev_spec("a...b").is_err(), "merge-base specs are refused");
        assert!(parse_rev_spec("a..").is_err());
        assert!(parse_rev_spec("..b").is_err());
    }

    #[test]
    fn since_takes_a_date_or_a_full_timestamp() {
        assert!(parse_since("2026-01-01").is_ok());
        assert!(parse_since("2026-01-01T00:00:00Z").is_ok());
        for bad in ["2026", "01-01-2026", "2026-01-01T00:00:00", "yesterday", ""] {
            assert!(parse_since(bad).is_err(), "should reject {bad}");
        }
    }

    #[test]
    fn shipped_excludes_anything_closed_without_shipping() {
        // The absence of a resolution is the whole rule: `done` alone does not mean
        // something shipped.
        let mk = |id: &str, res: &str| {
            issue(&format!(
                r#"{{"id": "{id}", "slug": "s", "title": "T", "status": "done", "priority": "low", "closed": "2026-06-11T00:00:00Z"{}}}"#,
                if res.is_empty() { String::new() } else { format!(r#", "resolution": "{res}""#) }
            ))
        };
        let rows = vec![mk("aaaaaaa", ""), mk("bbbbbbb", "wontfix"), issue(BASE)];
        let shipped = select_shipped(&rows, "2026-06-01");
        assert_eq!(shipped.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(), ["aaaaaaa"]);
    }

    #[test]
    fn an_empty_changelog_says_none() {
        let out = render_changelog(&[], "2026-01-01");
        assert!(out.contains("0 issues"), "{out}");
        assert!(out.ends_with("_none_\n"), "{out}");
    }
}

//! The mutating verbs, and the write path every one of them ends in.
//!
//! Two things are shared by all of them and worth reading first.
//!
//! [`finalize`] is the single write path: derive what is derived, write the index, write
//! the summary. Deriving on write rather than in each verb is what makes the rollup
//! uniform across `mv`, `start`, `done`, `new --parent` and re-parenting with no
//! per-command hooks.
//!
//! Writes are **atomic**: a temporary file in the same directory, then a rename. An
//! interrupted run leaves the previous index intact rather than half a line, which
//! matters because the index is the tracker's only source of truth.

use crate::config::{self, is_terminal};
use crate::discovery::Ctx;
use crate::graph::Graph;
use crate::id;
use crate::index::{parse_index, render_index};
use crate::issue::{DEFAULT_POINTS, Issue, check_field_key};
use crate::render::python_list;
use crate::summary::{filename, generate_summary};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The prose skeleton a new issue's body starts from.
const TEMPLATE: &str = "# {title}\n\
    \n\
    ## Summary\n\
    <!-- What needs doing and why. For an epic, link the spec instead of re-narrating it. -->\n\
    \n\
    ## Acceptance criteria\n\
    - [ ]\n\
    \n\
    ## Notes\n\
    <!-- Context, links to files/commits, open questions, decisions. -->\n";

/// The stamp written to `created`/`started`/`closed`.
///
/// `TRCK_NOW` overrides the clock, which is what makes a sequence of commands
/// reproducible for the conformance suite. Read per call, so a fixture can advance it
/// between invocations. A malformed value is an error rather than a fall back to the
/// real clock — falling back would make a fixture pass locally and fail elsewhere for a
/// reason nothing in the output explains.
pub(crate) fn now_utc() -> Result<String, String> {
    match std::env::var("TRCK_NOW") {
        Ok(v) if !v.is_empty() => parse_instant(&v),
        _ => Ok(system_now()),
    }
}

/// Seconds since the Unix epoch, rendered as the engine's canonical stamp.
fn system_now() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    format_epoch(i64::try_from(secs).unwrap_or(0))
}

/// Civil date-time from a Unix timestamp. Written out because the standard library has
/// no calendar; the algorithm is the usual days-from-civil inverse.
// The calendar arithmetic below is Howard Hinnant's days-from-civil algorithm, kept in
// its published single-letter form. Renaming `y`/`m`/`d`/`doe`/`yoe` to something
// "clearer" would make it unverifiable against the reference for no reader's benefit.
#[allow(
    clippy::many_single_char_names,
    reason = "matches the published algorithm"
)]
fn format_epoch(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (h, mi, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
}

/// Accept any ISO-8601 instant and normalise it to the one shape the engine writes.
/// A day-only value is refused: those are a legacy form the engine no longer emits, and
/// expanding one to midnight would reintroduce them through the back door.
#[allow(
    clippy::many_single_char_names,
    reason = "matches the published algorithm"
)]
fn parse_instant(v: &str) -> Result<String, String> {
    let bad =
        || format!("TRCK_NOW='{v}' is not an ISO-8601 instant (want e.g. 2026-01-01T00:00:00Z)");
    let (date, rest) = v.split_once('T').ok_or_else(|| {
        if v.len() == 10 && v.split('-').count() == 3 {
            format!("TRCK_NOW='{v}' is a date, not an instant (want e.g. 2026-01-01T00:00:00Z)")
        } else {
            bad()
        }
    })?;
    let nums: Vec<i64> = date
        .split('-')
        .map(|p| p.parse().map_err(|_| bad()))
        .collect::<Result<_, _>>()?;
    let [y, m, d] = nums[..] else {
        return Err(bad());
    };
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return Err(bad());
    }
    // Offset handling: strip it, then apply it in seconds.
    let (clock, offset) = split_offset(rest).ok_or_else(bad)?;
    let hms: Vec<i64> = clock
        .split(':')
        .map(|p| p.split('.').next().unwrap_or(p).parse().map_err(|_| bad()))
        .collect::<Result<_, _>>()?;
    let [h, mi, s] = hms[..] else {
        return Err(bad());
    };
    if h > 23 || mi > 59 || s > 60 {
        return Err(bad());
    }
    Ok(format_epoch(
        days_from_civil(y, m, d) * 86_400 + h * 3600 + mi * 60 + s - offset,
    ))
}

/// `(clock, offset_seconds)` from the part after `T`.
fn split_offset(rest: &str) -> Option<(&str, i64)> {
    if let Some(clock) = rest.strip_suffix('Z') {
        return Some((clock, 0));
    }
    for (i, c) in rest.char_indices().skip(1) {
        if c == '+' || c == '-' {
            let (clock, off) = rest.split_at(i);
            let sign = if c == '-' { -1 } else { 1 };
            let (hh, mm) = off[1..].split_once(':')?;
            let h: i64 = hh.parse().ok()?;
            let m: i64 = mm.parse().ok()?;
            return Some((clock, sign * (h * 3600 + m * 60)));
        }
    }
    Some((rest, 0)) // naive: treated as UTC
}

#[allow(
    clippy::many_single_char_names,
    reason = "matches the published algorithm"
)]
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// A filesystem-safe slug from a title: lowercase, runs of non-alphanumerics collapsed
/// to a single dash, trimmed.
pub(crate) fn slugify(text: &str) -> String {
    let mut out = String::new();
    let mut pending_dash = false;
    for c in text.chars() {
        if c.is_ascii_alphanumeric() {
            if pending_dash && !out.is_empty() {
                out.push('-');
            }
            pending_dash = false;
            out.push(c.to_ascii_lowercase());
        } else if c.is_ascii() || !c.is_alphanumeric() {
            pending_dash = true;
        } else {
            // Non-ASCII alphanumerics are not filesystem-safe across platforms and
            // Python's slugify drops them too.
            pending_dash = true;
        }
    }
    out
}

/// Whether a slug is usable as a filename component.
pub(crate) fn check_slug(slug: &str) -> bool {
    let mut chars = slug.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

pub(crate) fn issue_path(ctx: &Ctx, row: &Issue) -> PathBuf {
    ctx.items_dir().join(filename(row))
}

/// Write a file by writing a sibling temporary and renaming over the target.
///
/// A rename within a directory is atomic on every platform trck runs on, so an
/// interrupted run leaves the previous contents rather than a truncated file. The index
/// is the tracker's only source of truth; half of one is worse than none.
pub(crate) fn write_file(path: &Path, contents: &str) -> Result<(), String> {
    write_atomic(path, contents)
}

fn write_atomic(path: &Path, contents: &str) -> Result<(), String> {
    let tmp = path.with_extension(format!("tmp{}", std::process::id()));
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    std::fs::write(&tmp, contents).map_err(|e| format!("{}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(())
}

/// Apply a status transition and stamp the dates it implies.
///
/// Pure — no filesystem contact — so it is safe wherever the working tree may not be
/// settled: in-memory normalisation, dry runs, merge drivers.
pub(crate) fn apply_status(row: &mut Issue, new_status: &str) -> Result<(), String> {
    if let Some(msg) = config::check_status(new_status) {
        return Err(msg);
    }
    let was_initial = row.status == config::initial_status();
    row.status = new_status.to_string();
    if was_initial && new_status != config::initial_status() && row.started.is_none() {
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

/// Rows ordered children-before-parents, so a bottom-up pass sees each node's
/// descendants already settled.
fn postorder(g: &Graph) -> Vec<String> {
    let mut out = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for r in &g.rows {
        // Explicit stack with a visit flag; recursion would blow up on the deep
        // hierarchy a malformed index can produce.
        let mut stack = vec![(r.id.clone(), false)];
        while let Some((id, expanded)) = stack.pop() {
            if expanded {
                out.push(id);
                continue;
            }
            if !seen.insert(id.clone()) {
                continue;
            }
            stack.push((id.clone(), true));
            for kid in g.children_of(&id) {
                stack.push((kid.clone(), false));
            }
        }
    }
    out
}

/// Persist, regenerate and derive. The single write path every mutating verb ends in.
///
/// The two normalisations happen here rather than in each verb, which is what makes the
/// rollup uniform: `points` is a leaf-only input, so a parent's is reset; and a parent's
/// status is derived from its children unless it is pinned with `manual_status`.
pub(crate) fn finalize(ctx: &Ctx, rows: Vec<Issue>) -> Result<Vec<Issue>, String> {
    let mut g = Graph::new(rows);

    let parent_ids: Vec<String> = g
        .rows
        .iter()
        .map(|r| r.id.clone())
        .filter(|id| !g.is_leaf(id))
        .collect();
    for r in &mut g.rows {
        if parent_ids.contains(&r.id) {
            r.points = DEFAULT_POINTS;
        }
    }

    for id in postorder(&g) {
        let kids = g.children_of(&id).to_vec();
        if kids.is_empty() {
            continue;
        }
        let Some(row) = g.get(&id) else { continue };
        if row.manual_status {
            continue;
        }
        let statuses: Vec<String> = kids
            .iter()
            .filter_map(|k| g.get(k).map(|r| r.status.clone()))
            .collect();
        let desired = config::reconcile(&statuses);
        if g.get(&id).is_some_and(|r| r.status != desired) {
            let mut rows = std::mem::take(&mut g.rows);
            if let Some(row) = rows.iter_mut().find(|r| r.id == id) {
                apply_status(row, desired)?;
            }
            g = Graph::new(rows);
        }
    }

    write_atomic(&ctx.index_path(), &render_index(&g.rows))?;
    write_atomic(&ctx.summary_path(), &generate_summary(&g))?;

    // Validate what was just written, reusing the rows rather than re-parsing. A verb
    // that leaves the tracker inconsistent still succeeds — it did what it was asked —
    // but says so loudly, because the next thing that runs is usually a commit.
    if let Ok(report) = crate::validate::validate(ctx, &g.rows) {
        for w in &report.warnings {
            eprintln!("warning: {w}");
        }
        if !report.errors.is_empty() {
            eprintln!("\nINCONSISTENCIES after this operation:");
            for e in &report.errors {
                eprintln!("  error: {e}");
            }
            eprintln!("the tracker is now inconsistent — fix before committing.");
        }
    }
    Ok(g.rows)
}

pub(crate) fn load_rows(ctx: &Ctx) -> Result<Vec<Issue>, String> {
    let path = ctx.index_path();
    match std::fs::read_to_string(&path) {
        Ok(text) => parse_index(&text, "index.jsonl"),
        Err(_) => Ok(Vec::new()),
    }
}

/// Resolve a CLI id token to exactly one issue: exact id, then unique prefix.
pub(crate) fn resolve_ref(rows: &[Issue], token: &str) -> Result<String, String> {
    let token = token.strip_prefix('#').unwrap_or(token);
    if rows.iter().any(|r| r.id == token) {
        return Ok(token.to_string());
    }
    let hits: Vec<&str> = rows
        .iter()
        .map(|r| r.id.as_str())
        .filter(|id| id.starts_with(token))
        .collect();
    match hits.len() {
        1 => Ok(hits[0].to_string()),
        0 => Err(format!("no issue matching '{token}'")),
        _ => {
            let mut cands = hits;
            cands.sort_unstable();
            Err(format!(
                "ambiguous id prefix '{token}' matches: {}",
                cands.join(", ")
            ))
        }
    }
}

// --------------------------------------------------------------------------- //
// the verbs
// --------------------------------------------------------------------------- //

/// Options `new` accepts. A struct rather than a long parameter list, because the CLI
/// layer fills it field by field and a positional call would be unreadable.
#[derive(Default)]
pub(crate) struct NewOpts {
    pub(crate) title: String,
    pub(crate) id: Option<String>,
    pub(crate) slug: Option<String>,
    pub(crate) priority: Option<String>,
    pub(crate) points: Option<i64>,
    pub(crate) parent: Option<String>,
    pub(crate) depends: Vec<String>,
    pub(crate) spec: Option<String>,
    pub(crate) review_url: Option<String>,
}

pub(crate) fn cmd_new(ctx: &Ctx, opts: &NewOpts) -> Result<String, String> {
    let mut rows = load_rows(ctx)?;
    let taken = taken_ids(ctx, &rows);
    let iid = if let Some(v) = &opts.id {
        // A supplied id clears the same bar as a generated one: valid, and unused in
        // the index *and* on disk.
        if let Some(msg) = id::check(v) {
            return Err(msg);
        }
        if taken.contains(v) {
            return Err(format!("id '{v}' is already taken"));
        }
        v.clone()
    } else {
        id::generate(&|c| taken.contains(c))
    };
    let slug = opts.slug.clone().unwrap_or_else(|| slugify(&opts.title));
    if !check_slug(&slug) {
        return Err(format!("computed slug '{slug}' is invalid; pass --slug"));
    }
    let priority = opts
        .priority
        .clone()
        .unwrap_or_else(|| config::default_priority().to_string());
    if let Some(msg) = config::check_priority(&priority) {
        return Err(msg);
    }
    let points = opts.points.unwrap_or(DEFAULT_POINTS);
    if let Some(msg) = config::check_points(points) {
        return Err(msg);
    }
    if let Some(url) = &opts.review_url
        && let Some(msg) = config::check_review_url(url)
    {
        return Err(msg);
    }
    let parent = opts
        .parent
        .as_ref()
        .map(|p| resolve_ref(&rows, p))
        .transpose()?;
    let depends: Vec<String> = opts
        .depends
        .iter()
        .map(|d| resolve_ref(&rows, d))
        .collect::<Result<_, _>>()?;

    let row = Issue {
        id: iid.clone(),
        slug,
        title: opts.title.clone(),
        status: config::initial_status().to_string(),
        priority,
        points,
        parent,
        labels: Vec::new(),
        depends_on: depends.clone(),
        spec: opts.spec.clone(),
        review_url: opts.review_url.clone(),
        created: Some(now_utc()?),
        started: None,
        closed: None,
        resolution: None,
        manual_status: false,
        extra: BTreeMap::new(),
    };
    let path = issue_path(ctx, &row);
    if path.exists() {
        return Err(format!("{} already exists", path.display()));
    }
    rows.push(row);

    // Guard the new node's edges against the candidate graph — its parent is already
    // set, so an inherited cousin cycle is caught — before writing anything.
    let g = Graph::new(rows);
    for dep in &depends {
        if let Some(msg) = g.check_dep_edge(&iid, dep) {
            return Err(msg);
        }
    }
    let rows = g.rows;
    write_atomic(&path, &TEMPLATE.replace("{title}", &opts.title))?;
    finalize(ctx, rows)?;
    Ok(path.display().to_string())
}

/// Every id visible: index rows plus on-disk filenames. A branch may carry a body file
/// whose index line has not merged yet, so checking only the index would let `--id`
/// reintroduce the collision random ids exist to prevent.
fn taken_ids(ctx: &Ctx, rows: &[Issue]) -> std::collections::BTreeSet<String> {
    let mut ids: std::collections::BTreeSet<String> = rows.iter().map(|r| r.id.clone()).collect();
    if let Ok(entries) = std::fs::read_dir(ctx.items_dir()) {
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if let Some(stem) = name.strip_suffix(".md")
                && let Some((id, _)) = stem.split_once('-')
            {
                ids.insert(id.to_string());
            }
        }
    }
    ids
}

pub(crate) fn cmd_mv(
    ctx: &Ctx,
    token: &str,
    status: &str,
    resolution: Option<&str>,
    review_url: Option<&str>,
) -> Result<String, String> {
    let mut rows = load_rows(ctx)?;
    let iid = resolve_ref(&rows, token)?;
    if let Some(res) = resolution {
        if !is_terminal(status) {
            return Err("--resolution is only valid when moving to a terminal status".into());
        }
        if let Some(msg) = config::check_resolution(res) {
            return Err(msg);
        }
    }
    if let Some(url) = review_url
        && let Some(msg) = config::check_review_url(url)
    {
        return Err(msg);
    }
    if let Some(msg) = config::check_status(status) {
        return Err(msg);
    }
    let path = {
        let g = Graph::new(std::mem::take(&mut rows));
        let p = g.get(&iid).map(|r| issue_path(ctx, r));
        rows = g.rows;
        p.ok_or_else(|| format!("no issue matching '{iid}'"))?
    };
    if !path.exists() {
        return Err(format!("file missing for #{iid}: {}", path.display()));
    }

    let kid_statuses: Vec<String> = {
        let g = Graph::new(std::mem::take(&mut rows));
        let ks = g
            .children_of(&iid)
            .iter()
            .filter_map(|k| g.get(k).map(|r| r.status.clone()))
            .collect();
        rows = g.rows;
        ks
    };
    if let Some(row) = rows.iter_mut().find(|r| r.id == iid) {
        apply_status(row, status)?;
        if let Some(url) = review_url {
            row.review_url = Some(url.to_string());
        }
        if let Some(res) = resolution {
            row.resolution = Some(res.to_string());
        }
        // Moving a node with children overrides the rollup — but only when the
        // requested status differs from what derivation would produce. A move that
        // agrees with the children leaves it unpinned, so nothing to override.
        if !kid_statuses.is_empty() {
            row.manual_status = row.status != config::reconcile(&kid_statuses);
        }
    }
    finalize(ctx, rows)?;
    Ok(path.display().to_string())
}

/// Apply `--field key=value` and `--unset key`. An empty value clears, as an alias for
/// `--unset`, so `--field assignee=` reads the way people expect.
fn apply_field_edits(row: &mut Issue, fields: &[&str], unset: &[&str]) -> Result<(), String> {
    for spec in fields {
        let (key, val) = spec
            .split_once('=')
            .ok_or_else(|| format!("--field expects key=value, got '{spec}'"))?;
        if let Some(msg) = check_field_key(key) {
            return Err(msg);
        }
        if val.is_empty() {
            row.extra.remove(key);
        } else {
            row.extra
                .insert(key.to_string(), crate::json::Json::String(val.to_string()));
        }
    }
    for key in unset {
        if let Some(msg) = check_field_key(key) {
            return Err(msg);
        }
        row.extra.remove(*key);
    }
    Ok(())
}

/// Options `set` accepts. `Option<&str>` throughout, because "not given" and "given as
/// `none`" mean different things: the first leaves a field alone, the second clears it.
#[derive(Default)]
pub(crate) struct SetOpts<'a> {
    pub(crate) auto: bool,
    pub(crate) priority: Option<&'a str>,
    pub(crate) points: Option<i64>,
    pub(crate) parent: Option<&'a str>,
    pub(crate) spec: Option<&'a str>,
    pub(crate) review_url: Option<&'a str>,
    pub(crate) title: Option<&'a str>,
    pub(crate) slug: Option<&'a str>,
    pub(crate) fields: Vec<&'a str>,
    pub(crate) unset: Vec<&'a str>,
}

pub(crate) fn cmd_set(ctx: &Ctx, token: &str, opts: &SetOpts) -> Result<String, String> {
    let mut rows = load_rows(ctx)?;
    let iid = resolve_ref(&rows, token)?;
    let g = Graph::new(std::mem::take(&mut rows));
    let is_leaf = g.is_leaf(&iid);
    let parent = opts
        .parent
        .filter(|p| *p != "none")
        .map(|p| resolve_ref(&g.rows, p))
        .transpose()?;
    rows = g.rows;

    let old_path = rows
        .iter()
        .find(|r| r.id == iid)
        .map(|r| issue_path(ctx, r))
        .ok_or_else(|| format!("no issue matching '{iid}'"))?;

    let Some(row) = rows.iter_mut().find(|r| r.id == iid) else {
        return Err(format!("no issue matching '{iid}'"));
    };
    if opts.auto {
        row.manual_status = false; // back to derivation; finalize re-derives and cascades
    }
    if let Some(p) = opts.priority {
        if let Some(msg) = config::check_priority(p) {
            return Err(msg);
        }
        row.priority = p.to_string();
    }
    if let Some(points) = opts.points {
        if let Some(msg) = config::check_points(points) {
            return Err(msg);
        }
        if !is_leaf {
            return Err(format!(
                "#{iid} has children; points is derived from them, not set"
            ));
        }
        row.points = points;
    }
    if opts.parent.is_some() {
        row.parent = parent;
    }
    if let Some(spec) = opts.spec {
        row.spec = (spec != "none").then(|| spec.to_string());
    }
    if let Some(url) = opts.review_url {
        if url != "none"
            && let Some(msg) = config::check_review_url(url)
        {
            return Err(msg);
        }
        row.review_url = (url != "none").then(|| url.to_string());
    }
    apply_field_edits(row, &opts.fields, &opts.unset)?;
    if let Some(slug) = opts.slug {
        if !check_slug(slug) {
            return Err(format!("invalid slug '{slug}'"));
        }
        row.slug = slug.to_string();
    }
    if let Some(title) = opts.title {
        row.title = title.to_string();
    }
    let new_path = rows
        .iter()
        .find(|r| r.id == iid)
        .map_or_else(|| old_path.clone(), |r| issue_path(ctx, r));

    // Re-parenting changes what is lifted, so it can introduce an effective cycle that
    // neither authored edge shows. Guard the candidate state before anything is written.
    if opts.parent.is_some() {
        let g = Graph::new(std::mem::take(&mut rows));
        let cycles = g.effective_cycles();
        let parent_cycles = g.parent_cycles();
        rows = g.rows;
        if let Some(cyc) = parent_cycles.first() {
            return Err(format!("parent cycle: {}", cyc.join(" -> ")));
        }
        if let Some(cyc) = cycles.first() {
            return Err(format!(
                "re-parenting #{iid} would create an effective dependency cycle: {}",
                cyc.join(" -> ")
            ));
        }
    }

    if old_path != new_path {
        std::fs::rename(&old_path, &new_path)
            .map_err(|e| format!("{} -> {}: {e}", old_path.display(), new_path.display()))?;
    }
    if let Some(title) = opts.title {
        retitle_body(&new_path, title)?;
    }
    finalize(ctx, rows)?;
    Ok(format!("#{iid} updated"))
}

/// Rewrite the body's first heading, so the file does not contradict the index. Only
/// the first line, and only when it is a heading — the rest is hand-authored prose.
fn retitle_body(path: &Path, title: &str) -> Result<(), String> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Ok(()); // a missing body is `check`'s business, not this verb's
    };
    let rewritten: Vec<String> = text
        .lines()
        .enumerate()
        .map(|(i, line)| {
            if i == 0 && line.starts_with("# ") {
                format!("# {title}")
            } else {
                line.to_string()
            }
        })
        .collect();
    let mut body = rewritten.join("\n");
    if text.ends_with('\n') {
        body.push('\n');
    }
    write_atomic(path, &body)
}

pub(crate) fn cmd_label(
    ctx: &Ctx,
    token: &str,
    add: &[&str],
    remove: &[&str],
) -> Result<String, String> {
    let mut rows = load_rows(ctx)?;
    let iid = resolve_ref(&rows, token)?;
    let Some(row) = rows.iter_mut().find(|r| r.id == iid) else {
        return Err(format!("no issue matching '{iid}'"));
    };
    for lab in add {
        if !lab.is_empty() && !row.labels.iter().any(|l| l == lab) {
            row.labels.push((*lab).to_string());
        }
    }
    row.labels.retain(|l| !remove.contains(&l.as_str()));
    row.labels.sort();
    let shown = python_list(&row.labels);
    finalize(ctx, rows)?;
    Ok(format!("#{iid} labels={shown}"))
}

pub(crate) fn cmd_dep(
    ctx: &Ctx,
    token: &str,
    add: Option<&str>,
    remove: Option<&str>,
) -> Result<String, String> {
    let mut rows = load_rows(ctx)?;
    let iid = resolve_ref(&rows, token)?;
    let add = add.map(|a| resolve_ref(&rows, a)).transpose()?;
    let remove = remove.map(|r| resolve_ref(&rows, r)).transpose()?;
    if let Some(target) = &add {
        let g = Graph::new(std::mem::take(&mut rows));
        let refusal = g.check_dep_edge(&iid, target);
        rows = g.rows;
        if let Some(msg) = refusal {
            return Err(msg);
        }
    }
    let Some(row) = rows.iter_mut().find(|r| r.id == iid) else {
        return Err(format!("no issue matching '{iid}'"));
    };
    if let Some(target) = add
        && !row.depends_on.contains(&target)
    {
        row.depends_on.push(target);
    }
    if let Some(target) = remove {
        row.depends_on.retain(|d| *d != target);
    }
    row.depends_on.sort();
    let shown = python_list(&row.depends_on);
    finalize(ctx, rows)?;
    Ok(format!("#{iid} depends_on={shown}"))
}

#[cfg(test)]
mod tests {
    // Tests assert; that is their job. The crate denies unwrap/expect/panic because a
    // malformed tracker must produce a diagnostic rather than a stack trace, but a test
    // that cannot panic cannot fail.
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    #[test]
    fn reopening_clears_both_closed_and_resolution() {
        // Leaving a terminal status must clear the whole closure record, not just the
        // timestamp: a row that is 'ongoing' while carrying a resolution is one our own
        // `check` rejects, so keeping it would have the verb write an invalid tracker.
        let mut row = Issue {
            id: "aaaaaaa".into(),
            slug: "alpha".into(),
            title: "Alpha".into(),
            status: config::DONE.into(),
            priority: "medium".into(),
            points: 1,
            parent: None,
            labels: Vec::new(),
            depends_on: Vec::new(),
            spec: None,
            review_url: None,
            created: Some("2026-01-01T00:00:00Z".into()),
            started: Some("2026-01-01T00:00:00Z".into()),
            closed: Some("2026-01-01T00:00:00Z".into()),
            resolution: Some("wontfix".into()),
            manual_status: false,
            extra: BTreeMap::new(),
        };
        apply_status(&mut row, config::ONGOING).unwrap();
        assert_eq!(row.closed, None);
        assert_eq!(
            row.resolution, None,
            "resolution must not outlive the closure"
        );
    }

    #[test]
    fn slugify_matches_the_python_rule() {
        assert_eq!(slugify("Fix the parser"), "fix-the-parser");
        assert_eq!(slugify("  Leading & trailing!  "), "leading-trailing");
        assert_eq!(slugify("CamelCase123"), "camelcase123");
        assert_eq!(slugify("---"), "");
        assert_eq!(slugify("café ✓"), "caf");
    }

    #[test]
    fn epoch_formatting_round_trips_known_instants() {
        assert_eq!(format_epoch(0), "1970-01-01T00:00:00Z");
        assert_eq!(format_epoch(1_767_225_600), "2026-01-01T00:00:00Z");
        assert_eq!(format_epoch(951_782_400), "2000-02-29T00:00:00Z");
    }

    #[test]
    fn trck_now_accepts_any_iso_instant_and_normalises_to_utc() {
        assert_eq!(
            parse_instant("2026-01-01T00:00:00Z").expect("ok"),
            "2026-01-01T00:00:00Z"
        );
        assert_eq!(
            parse_instant("2026-01-01T09:00:00+03:00").expect("ok"),
            "2026-01-01T06:00:00Z"
        );
    }

    #[test]
    fn trck_now_refuses_a_day_only_or_malformed_value() {
        // Refused rather than ignored: falling back to the real clock would make a
        // fixture pass locally and fail elsewhere for no visible reason.
        assert!(
            parse_instant("2026-01-01")
                .expect_err("refused")
                .contains("not an instant")
        );
        for bad in ["yesterday", "1735689600", "2026-13-01T00:00:00Z", "x"] {
            assert!(parse_instant(bad).is_err(), "should reject {bad}");
        }
    }

    #[test]
    fn resolve_ref_takes_an_exact_id_then_a_unique_prefix() {
        let mk = |id: &str| {
            crate::issue::Issue::from_json(
                &crate::json::parse(&format!(
                    r#"{{"id": "{id}", "slug": "s", "title": "T", "status": "backlog", "priority": "low"}}"#
                ))
                .expect("json"),
            )
            .expect("issue")
        };
        let rows = vec![mk("aaaaaaa"), mk("aabbbbb")];
        assert_eq!(resolve_ref(&rows, "aaaaaaa").expect("exact"), "aaaaaaa");
        assert_eq!(resolve_ref(&rows, "#aaaaaaa").expect("hash"), "aaaaaaa");
        assert_eq!(resolve_ref(&rows, "aab").expect("prefix"), "aabbbbb");
        assert!(
            resolve_ref(&rows, "aa")
                .expect_err("ambiguous")
                .contains("ambiguous")
        );
        assert!(
            resolve_ref(&rows, "zz")
                .expect_err("none")
                .contains("no issue")
        );
    }
}

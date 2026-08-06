//! The `repo` verbs: git integration and one-shot tracker maintenance.
//!
//! These differ in kind from the read and write verbs. Those operate on a settled tracker;
//! these write into `.git`, and the merge drivers **run inside a merge**, where the working
//! tree is not yet the merged result. `merge-index`'s contract is not "produces the right
//! file" but "produces the right file given three inputs git hands it mid-operation", and
//! its behaviour under conflict is part of that contract.

use crate::discovery::Ctx;
use crate::graph::Graph;
use crate::index::{parse_index, render_index};
use crate::json::Json;
use crate::merge::{conflict_ids, merge_rows};
use crate::summary::generate_summary;
use crate::verbs::write_atomic;
use std::path::Path;

/// Run a git command in the tracker directory, returning its trimmed stdout.
fn git(ctx: &Ctx, args: &[&str]) -> Result<String, String> {
    let out = std::process::Command::new("git")
        .args(args)
        .current_dir(&ctx.dir)
        .output()
        .map_err(|e| format!("running git: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// How a git driver should re-invoke this engine.
///
/// The absolute path of the running binary, never a bare `trck`. The driver command is
/// baked into `.git/config` and fires much later, from whatever environment git happens to
/// have: a `PATH` lookup need not resolve at all (a CI checkout installs nothing) and, where
/// it does, need not be this engine or this version. An absolute path is answerable now.
///
/// Unlike the Python engine this needs no interpreter prefix — the binary is the artifact —
/// and for the same reason there is no vendored-copy case: a vendored `trck` beside the
/// tracker is a Python script, which this engine cannot claim to be.
fn engine_invocation() -> Result<String, String> {
    let exe = std::env::current_exe().map_err(|e| format!("locating this binary: {e}"))?;
    Ok(format!("\"{}\"", exe.display()))
}

const GITATTRIBUTES_HEADER: &str = "# Managed by `trck repo setup-git` — trck merge drivers.";
const GITATTRIBUTES_LINES: &[&str] = &[
    "index.jsonl merge=trck-index",
    "SUMMARY.md merge=trck-summary",
];

/// `repo setup-git` — declare trck's merge drivers and register them in this clone.
///
/// Two halves, because git separates them on purpose: `<tracker>/.gitattributes` *names* the
/// drivers and is committed, so it is shared; `.git/config` *defines* what they run and is
/// per-clone, never shared — otherwise cloning a repo would be remote code execution.
///
/// So this runs once per clone. Until it does, git falls back to an ordinary 3-way merge
/// with normal conflict markers: an un-set-up clone is exactly as well off as before, which
/// is what lets this roll out gradually.
pub(crate) fn cmd_setup_git(ctx: &Ctx) -> Result<String, String> {
    git(ctx, &["rev-parse", "--git-common-dir"]).map_err(|_| "not a git repository".to_string())?;
    let mut out: Vec<String> = Vec::new();

    // --- shared half: name the drivers ---
    let path = ctx.dir.join(".gitattributes");
    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    let have: Vec<&str> = existing.lines().collect();
    let missing: Vec<&str> = GITATTRIBUTES_LINES
        .iter()
        .filter(|l| !have.contains(l))
        .copied()
        .collect();
    if missing.is_empty() {
        out.push(format!(
            "{} already declares the trck drivers",
            path.display()
        ));
    } else {
        let mut lines: Vec<String> = have.iter().map(|s| (*s).to_string()).collect();
        if lines.last().is_some_and(|l| !l.trim().is_empty()) {
            lines.push(String::new());
        }
        lines.push(GITATTRIBUTES_HEADER.to_string());
        lines.extend(missing.iter().map(|s| (*s).to_string()));
        write_atomic(&path, &(lines.join("\n") + "\n"))?;
        out.push(format!("wrote {}", path.display()));
    }

    // --- per-clone half: define what they run ---
    let engine = engine_invocation()?;
    let drivers = [
        (
            "trck-index",
            format!("{engine} repo merge-index %O %A %B"),
            "trck index.jsonl row-wise 3-way merge",
        ),
        (
            "trck-summary",
            format!("{engine} repo merge-summary %A"),
            "trck SUMMARY.md regeneration",
        ),
    ];
    for (name, cmd, label) in &drivers {
        git(ctx, &["config", &format!("merge.{name}.driver"), cmd])?;
        git(ctx, &["config", &format!("merge.{name}.name"), label])?;
    }
    out.push("registered merge drivers in this clone (trck-index, trck-summary)".into());
    out.push(
        "note: .gitattributes is shared, but the driver commands are per-clone — every \
         clone must run `trck repo setup-git` for auto-resolution to apply."
            .into(),
    );
    Ok(out.join("\n"))
}

/// `repo install-hook` — a pre-commit hook that runs `trck check` when the tracker changes.
pub(crate) fn cmd_install_hook(ctx: &Ctx) -> Result<String, String> {
    let common = git(ctx, &["rev-parse", "--git-common-dir"])
        .map_err(|_| "not a git repository".to_string())?;
    let toplevel = git(ctx, &["rev-parse", "--show-toplevel"])
        .map_err(|_| "not a git repository".to_string())?;
    let root = std::path::Path::new(&toplevel)
        .canonicalize()
        .map_err(|e| format!("{toplevel}: {e}"))?;
    let dir = ctx
        .dir
        .canonicalize()
        .map_err(|e| format!("{}: {e}", ctx.dir.display()))?;
    let rel = dir.strip_prefix(&root).map_err(|_| {
        format!(
            "tracker dir {} is not inside the git repo at {}",
            ctx.dir.display(),
            root.display()
        )
    })?;
    let rel = if rel.as_os_str().is_empty() {
        ".".to_string()
    } else {
        rel.to_string_lossy().replace('\\', "/")
    };

    let hooks = ctx.dir.join(&common);
    let hooks = hooks.canonicalize().unwrap_or(hooks).join("hooks");
    std::fs::create_dir_all(&hooks).map_err(|e| format!("{}: {e}", hooks.display()))?;
    let hook = hooks.join("pre-commit");

    // When the tracker dir IS the repo root, `rel` is "." and a path-prefix grep would never
    // match git's repo-relative staged paths — the hook would silently never run. There the
    // whole repo is the tracker, so fire on any staged change.
    let (guard, edir) = if rel == "." {
        (
            "if [ -n \"$staged\" ]; then".to_string(),
            "$root".to_string(),
        )
    } else {
        (
            format!(
                "if printf '%s\\n' \"$staged\" | grep -qE '(^|/){}/'; then",
                rel.replace('.', "\\.")
            ),
            format!("$root/{rel}"),
        )
    };
    let engine = engine_invocation()?;
    let body = format!(
        "#!/usr/bin/env bash\n\
         # Auto-installed by `trck repo install-hook`. Runs `trck check` when the tracker changes.\n\
         root=\"$(git rev-parse --show-toplevel)\"\n\
         staged=\"$(git diff --cached --name-only)\"\n\
         {guard}\n  \
           if [ -x {engine} ]; then\n    \
             {engine} --dir \"{edir}\" check || {{ echo \"trck inconsistent — aborting commit\"; exit 1; }}\n  \
           elif command -v trck >/dev/null 2>&1; then\n    \
             trck --dir \"{edir}\" check || {{ echo \"trck inconsistent — aborting commit\"; exit 1; }}\n  \
           fi\n\
         fi\n"
    );
    write_atomic(&hook, &body)?;
    make_executable(&hook)?;
    Ok(format!("installed {}", hook.display()))
}

/// Mark a file user-executable, which a hook must be for git to run it.
#[cfg(unix)]
fn make_executable(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
        .map_err(|e| format!("{}: {e}", path.display()))
}

// The `Result` is unnecessary on this platform, and deliberately kept: the two `cfg`
// arms are one function to every caller, and the unix arm can genuinely fail.
#[cfg(not(unix))]
#[allow(clippy::unnecessary_wraps)]
fn make_executable(_path: &Path) -> Result<(), String> {
    Ok(()) // Windows has no executable bit; git runs hooks through the shell there.
}

/// `repo normalize` — rewrite `index.jsonl` in canonical slim form.
///
/// No data change: it re-serialises through the same write path every verb ends in, which
/// also regenerates the summary and re-derives what is derived.
pub(crate) fn cmd_normalize(ctx: &Ctx) -> Result<String, String> {
    let text = std::fs::read_to_string(ctx.index_path()).unwrap_or_default();
    let rows = parse_index(&text, "index.jsonl")?;
    let n = rows.len();
    crate::verbs::finalize(ctx, rows)?;
    Ok(format!(
        "normalized {} ({n} issues)",
        ctx.index_path().display()
    ))
}

/// `repo migrate-layout [--dry-run]` — move issue bodies out of per-status folders.
///
/// One-shot and idempotent: a flat tracker is a no-op. Deliberately conservative about the
/// one ambiguity a legacy tracker can carry — if a file's folder disagrees with its index
/// status, the two sources of truth have already drifted and only the author knows which is
/// right, so it stops rather than silently canonising one.
pub(crate) fn cmd_migrate_layout(ctx: &Ctx, dry_run: bool) -> Result<String, String> {
    let stale = crate::discovery::legacy_layout_files(&ctx.dir);
    if stale.is_empty() {
        return Ok(format!(
            "migrate-layout: nothing to migrate (already flat in {}/)",
            crate::discovery::ITEMS_DIR
        ));
    }
    let text = std::fs::read_to_string(ctx.index_path()).unwrap_or_default();
    let rows = parse_index(&text, "index.jsonl")?;
    let dest_dir = ctx.items_dir();

    let (mut drift, mut collisions, mut moves) = (Vec::new(), Vec::new(), Vec::new());
    for p in &stale {
        let name = p
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let iid = name.split('-').next().unwrap_or_default().to_string();
        let folder = p
            .parent()
            .and_then(|d| d.file_name())
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        if let Some(row) = rows.iter().find(|r| r.id == iid)
            && row.status != folder
        {
            drift.push(format!(
                "#{iid}: index says '{}', file sits in '{folder}/'",
                row.status
            ));
            continue;
        }
        let dest = dest_dir.join(&name);
        if dest.exists() {
            collisions.push(format!("#{iid}: {} already exists", dest.display()));
            continue;
        }
        moves.push((p.clone(), dest, folder, name));
    }

    if !drift.is_empty() {
        return Err(format!(
            "index status and folder disagree for {} issue(s); fix the index (or move the \
             files) so they agree, then re-run:\n  {}",
            drift.len(),
            drift.join("\n  ")
        ));
    }
    if !collisions.is_empty() {
        return Err(format!(
            "destination already occupied for {} file(s):\n  {}",
            collisions.len(),
            collisions.join("\n  ")
        ));
    }

    if dry_run {
        let mut out = vec![format!(
            "migrate-layout: would move {} file(s) into {}/",
            moves.len(),
            crate::discovery::ITEMS_DIR
        )];
        for (_, _, folder, name) in &moves {
            out.push(format!(
                "  {folder}/{name} -> {}/{name}",
                crate::discovery::ITEMS_DIR
            ));
        }
        return Ok(out.join("\n"));
    }

    std::fs::create_dir_all(&dest_dir).map_err(|e| format!("{}: {e}", dest_dir.display()))?;
    for (src, dest, _, _) in &moves {
        std::fs::rename(src, dest)
            .map_err(|e| format!("{} -> {}: {e}", src.display(), dest.display()))?;
    }
    // Drop the status folders that are now empty. One holding anything else — a README, a
    // scratch note — is left alone, which `remove_dir` gives for free by refusing.
    let mut folders: Vec<&Path> = moves.iter().filter_map(|(s, ..)| s.parent()).collect();
    folders.sort_unstable();
    folders.dedup();
    for folder in folders {
        let _ = std::fs::remove_dir(folder);
    }
    Ok(format!(
        "migrate-layout: moved {} file(s) into {}/",
        moves.len(),
        crate::discovery::ITEMS_DIR
    ))
}

/// Read an index file into raw JSON rows.
///
/// A merge operand is not validated on the way in: it may hold a row this engine would
/// reject, and the merge still has to describe the disagreement rather than die on the
/// parse. A missing file is an empty side — git passes `/dev/null`-like empties for a
/// creation on one branch.
fn read_rows(path: &str) -> Result<Vec<Json>, String> {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let mut out = Vec::new();
    for (n, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        out.push(crate::json::parse(line).map_err(|e| format!("{path} line {}: {e}", n + 1))?);
    }
    Ok(out)
}

/// `repo merge-index BASE CURRENT OTHER` — the git merge driver for `index.jsonl`.
///
/// Git passes `%O` (common ancestor), `%A`, `%B` and takes the contents of `%A` as the
/// result; exit 0 means resolved, non-zero conflicted.
///
/// On a clean merge this also regenerates `SUMMARY.md` **from the merged rows**, not by
/// re-reading the working-tree index, which during a merge is not yet the merged result.
/// That is what makes the driver-ordering question moot: git gives no ordering guarantee
/// between per-file drivers, so whichever runs first, the rollup derives from the same rows.
///
/// On a conflict it writes marker blocks and leaves `SUMMARY.md` alone. A rollup regenerated
/// from a half-merged index would launder the conflict into a plausible-looking file; a
/// stale rollup is obvious, a fabricated one is not.
pub(crate) fn cmd_merge_index(
    ctx: Option<&Ctx>,
    base: &str,
    current: &str,
    other: &str,
) -> Result<String, String> {
    let (base_rows, a_rows, b_rows) = (read_rows(base)?, read_rows(current)?, read_rows(other)?);
    let (rows, conflicts) = merge_rows(&base_rows, &a_rows, &b_rows)?;
    let dest = Path::new(current);

    if conflicts.is_empty() {
        write_atomic(dest, &render_index(&rows))?;
        if let Some(ctx) = ctx {
            let g = Graph::new(rows);
            write_atomic(&ctx.summary_path(), &generate_summary(&g))?;
        }
        return Ok(String::new());
    }

    // Conflicted: the clean rows plus a marker block per conflicting id, so the file cannot
    // be parsed — and therefore cannot be `git add`ed unread — until a human resolves it.
    // Sides are labelled by position, never by ownership.
    let bad = conflict_ids(&conflicts);
    let by = |rows: &[Json], id: &str| -> Option<Json> {
        rows.iter()
            .find(|r| r.get("id").and_then(Json::as_str) == Some(id))
            .cloned()
    };
    let mut out: Vec<String> = rows
        .iter()
        .filter(|r| !bad.contains(&r.id))
        .map(|r| r.to_canonical().to_json())
        .collect();
    for iid in &bad {
        out.push(format!("<<<<<<< one side ({iid})"));
        if let Some(r) = by(&a_rows, iid) {
            out.push(r.to_json());
        }
        out.push("=======".into());
        if let Some(r) = by(&b_rows, iid) {
            out.push(r.to_json());
        }
        out.push(format!(">>>>>>> the other side ({iid})"));
    }
    write_atomic(dest, &(out.join("\n") + "\n"))?;

    // Self-labelled, so `main` prints it verbatim: git shows a driver's stderr to the user
    // as-is, and this is a whole report — headline, the conflicts, then what to do next.
    let mut lines = vec![format!(
        "trck: index.jsonl has {} unresolved conflict(s):",
        conflicts.len()
    )];
    lines.extend(conflicts.iter().map(|c| format!("  {c}")));
    lines.push("resolve the marked rows, then `git add` and re-run `trck check`.".into());
    Err(lines.join("\n"))
}

/// `repo merge-summary` — discard both sides and regenerate.
///
/// The rollup derives entirely from `index.jsonl`, so there is never anything to merge. This
/// is a safety net rather than the authority: `merge-index` already rewrites `SUMMARY.md`
/// from the rows it merged, which is what makes the order git runs the drivers in irrelevant.
pub(crate) fn cmd_merge_summary(ctx: Option<&Ctx>, current: &str) -> Result<String, String> {
    let Some(ctx) = ctx else {
        // No tracker in reach: leave the file alone rather than truncate it. Reporting
        // success here would tell git a merge resolved when nothing was written.
        return Err("merge-summary: no tracker found to regenerate from".into());
    };
    let text = std::fs::read_to_string(ctx.index_path()).unwrap_or_default();
    let rows = parse_index(&text, "index.jsonl")?;
    let g = Graph::new(rows);
    write_atomic(Path::new(current), &generate_summary(&g))?;
    Ok(String::new())
}

//! `repo migrate-layout [--dry-run]` — move issue bodies out of per-status folders.
//!
//! One-shot and idempotent: a flat tracker is a no-op. Deliberately conservative about the
//! one ambiguity a legacy tracker can carry — if a file's folder disagrees with its index
//! status, the two sources of truth have already drifted and only the author knows which is
//! right, so it stops rather than silently canonising one.

use crate::discovery::{Ctx, ITEMS_DIR};
use crate::index::parse_index;
use crate::issue::Issue;
use std::path::{Path, PathBuf};

pub(crate) fn cmd_migrate_layout(ctx: &Ctx, dry_run: bool) -> Result<String, String> {
    let stale = crate::discovery::legacy_layout_files(&ctx.dir);
    if stale.is_empty() {
        return Ok(format!("migrate-layout: nothing to migrate (already flat in {ITEMS_DIR}/)"));
    }
    let text = std::fs::read_to_string(ctx.index_path()).unwrap_or_default();
    let rows = parse_index(&text, "index.jsonl")?;
    let dest_dir = ctx.items_dir();

    let plan = Plan::new(&stale, &rows, &dest_dir);
    plan.refuse()?;
    if dry_run {
        return Ok(plan.preview());
    }
    plan.apply(&dest_dir)
}

/// One file's move, and where it came from — the folder is kept for the dry-run listing, which
/// has to name the move the way the operator will recognise it.
struct Move {
    src: PathBuf,
    dest: PathBuf,
    folder: String,
    name: String,
}

/// Every legacy file sorted into one of three piles. Nothing is written while this is built, so
/// a tracker that turns out to be ambiguous is left exactly as it was.
struct Plan {
    moves: Vec<Move>,
    drift: Vec<String>,
    collisions: Vec<String>,
}

impl Plan {
    fn new(stale: &[PathBuf], rows: &[Issue], dest_dir: &Path) -> Plan {
        let mut plan = Plan { moves: Vec::new(), drift: Vec::new(), collisions: Vec::new() };
        for src in stale {
            plan.sort_one(src, rows, dest_dir);
        }
        plan
    }

    /// Which pile one file belongs in: drifted, blocked, or movable.
    fn sort_one(&mut self, src: &Path, rows: &[Issue], dest_dir: &Path) {
        let name = src.file_name().unwrap_or_default().to_string_lossy().to_string();
        let iid = name.split('-').next().unwrap_or_default().to_string();
        let folder = src.parent().and_then(|d| d.file_name()).unwrap_or_default().to_string_lossy().to_string();
        if let Some(row) = rows.iter().find(|r| r.id == iid)
            && row.status != folder
        {
            self.drift.push(format!("#{iid}: index says '{}', file sits in '{folder}/'", row.status));
            return;
        }
        let dest = dest_dir.join(&name);
        if dest.exists() {
            self.collisions.push(format!("#{iid}: {} already exists", dest.display()));
            return;
        }
        self.moves.push(Move { src: src.to_path_buf(), dest, folder, name });
    }

    /// Refuse rather than guess, and name every case so one run tells the author everything
    /// they have to fix.
    fn refuse(&self) -> Result<(), String> {
        if !self.drift.is_empty() {
            return Err(format!(
                "index status and folder disagree for {} issue(s); fix the index (or move the \
                 files) so they agree, then re-run:\n  {}",
                self.drift.len(),
                self.drift.join("\n  ")
            ));
        }
        if !self.collisions.is_empty() {
            return Err(format!("destination already occupied for {} file(s):\n  {}", self.collisions.len(), self.collisions.join("\n  ")));
        }
        Ok(())
    }

    /// `--dry-run`: every move named, so they can be checked before anything is touched.
    fn preview(&self) -> String {
        let mut out = vec![format!("migrate-layout: would move {} file(s) into {ITEMS_DIR}/", self.moves.len())];
        for m in &self.moves {
            out.push(format!("  {}/{} -> {ITEMS_DIR}/{}", m.folder, m.name, m.name));
        }
        out.join("\n")
    }

    /// Move them, then drop the status folders that are now empty. One holding anything else —
    /// a README, a scratch note — is left alone, which `remove_dir` gives for free by refusing.
    fn apply(&self, dest_dir: &Path) -> Result<String, String> {
        std::fs::create_dir_all(dest_dir).map_err(|e| format!("{}: {e}", dest_dir.display()))?;
        for m in &self.moves {
            std::fs::rename(&m.src, &m.dest).map_err(|e| format!("{} -> {}: {e}", m.src.display(), m.dest.display()))?;
        }
        let mut folders: Vec<&Path> = self.moves.iter().filter_map(|m| m.src.parent()).collect();
        folders.sort_unstable();
        folders.dedup();
        for folder in folders {
            let _ = std::fs::remove_dir(folder);
        }
        Ok(format!("migrate-layout: moved {} file(s) into {ITEMS_DIR}/", self.moves.len()))
    }
}

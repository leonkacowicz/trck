//! `repo setup-git` — declare trck's merge drivers and register them in this clone.

use super::attributes::gitattributes_update;
use super::git::{engine_invocation, git, require_repo};
use crate::discovery::{Ctx, Source};
use crate::verbs::write_atomic;
use std::path::Path;

/// Two halves, because git separates them on purpose: `<tracker>/.gitattributes` *names* the
/// drivers and is committed, so it is shared; `.git/config` *defines* what they run and is
/// per-clone, never shared — otherwise cloning a repo would be remote code execution.
///
/// So this runs once per clone. Until it does, git falls back to an ordinary 3-way merge
/// with normal conflict markers: an un-set-up clone is exactly as well off as before, which
/// is what lets this roll out gradually.
pub(crate) fn cmd_setup_git(invocation_cwd: &Path, context: Option<&Ctx>) -> Result<String, String> {
    let source = context.map(|ctx| &ctx.source);
    let cwd = source.map_or(invocation_cwd, |source| match source {
        Source::Dir(dir) => dir,
        Source::Ref { cwd, .. } => cwd,
    });
    require_repo(cwd, "--git-common-dir")?;
    let mut out: Vec<String> = vec![declare(source)?];
    register(cwd)?;
    out.push("registered merge drivers in this clone (trck-index, trck-summary)".into());
    out.push(widen_refspec(cwd)?);
    out.push(
        "note: .gitattributes is shared, but the driver commands are per-clone — every \
         clone must run `trck repo setup-git` for auto-resolution to apply."
            .into(),
    );
    Ok(out.join("\n"))
}

/// The shared half: name the drivers in the committed `.gitattributes`.
fn declare(source: Option<&Source>) -> Result<String, String> {
    let dir = match source {
        Some(Source::Dir(dir)) => dir,
        Some(Source::Ref { rev, .. }) => {
            return Ok(format!("skipped .gitattributes: the tracker is stored in git ref '{rev}', which has no working-tree file"));
        },
        None => return Ok("skipped .gitattributes: no tracker is available in the working tree".into()),
    };
    let path = dir.join(".gitattributes");
    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    let have: Vec<&str> = existing.lines().collect();
    let Some(lines) = gitattributes_update(&have) else {
        return Ok(format!("{} already declares the trck drivers", path.display()));
    };
    write_atomic(&path, &(lines.join("\n") + "\n"))?;
    Ok(format!("wrote {}", path.display()))
}

/// Make sure this clone actually fetches the tracker branch.
///
/// The same per-clone problem as the drivers, and the same verb solves it: a shallow or
/// single-branch clone fetches one branch, so `origin/trck-issues` never arrives and the
/// tracker reads as absent. Idempotent, because the check is whether the configured
/// refspecs already cover the branch rather than whether this ever ran.
fn widen_refspec(cwd: &Path) -> Result<String, String> {
    use crate::discovery::refspec::{configured, covered, tracker_refspec};
    let branch = crate::discovery::TRACKER_REF;
    if covered(&configured(cwd, "origin"), branch) {
        return Ok(format!("this clone already fetches {branch}"));
    }
    let spec = tracker_refspec(branch);
    git(cwd, &["config", "--add", "remote.origin.fetch", &spec])?;
    Ok(format!("added `{spec}` to remote.origin.fetch — run `git fetch` to bring the branch in"))
}

/// The per-clone half: define what the named drivers actually run.
fn register(cwd: &Path) -> Result<(), String> {
    let engine = engine_invocation()?;
    let drivers = [
        ("trck-index", format!("{engine} repo merge-index %O %A %B"), "trck index.jsonl row-wise 3-way merge"),
        ("trck-summary", format!("{engine} repo merge-summary %A"), "trck SUMMARY.md regeneration"),
    ];
    for (name, cmd, label) in &drivers {
        git(cwd, &["config", &format!("merge.{name}.driver"), cmd])?;
        git(cwd, &["config", &format!("merge.{name}.name"), label])?;
    }
    Ok(())
}

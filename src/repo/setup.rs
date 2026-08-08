//! `repo setup-git` — declare trck's merge drivers and register them in this clone.

use super::attributes::gitattributes_update;
use super::git::{engine_invocation, git, require_repo};
use crate::discovery::Ctx;
use crate::verbs::write_atomic;

/// Two halves, because git separates them on purpose: `<tracker>/.gitattributes` *names* the
/// drivers and is committed, so it is shared; `.git/config` *defines* what they run and is
/// per-clone, never shared — otherwise cloning a repo would be remote code execution.
///
/// So this runs once per clone. Until it does, git falls back to an ordinary 3-way merge
/// with normal conflict markers: an un-set-up clone is exactly as well off as before, which
/// is what lets this roll out gradually.
pub(crate) fn cmd_setup_git(ctx: &Ctx) -> Result<String, String> {
    require_repo(ctx, "--git-common-dir")?;
    let mut out: Vec<String> = vec![declare(ctx)?];
    register(ctx)?;
    out.push("registered merge drivers in this clone (trck-index, trck-summary)".into());
    out.push(
        "note: .gitattributes is shared, but the driver commands are per-clone — every \
         clone must run `trck repo setup-git` for auto-resolution to apply."
            .into(),
    );
    Ok(out.join("\n"))
}

/// The shared half: name the drivers in the committed `.gitattributes`.
fn declare(ctx: &Ctx) -> Result<String, String> {
    let path = ctx.dir.join(".gitattributes");
    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    let have: Vec<&str> = existing.lines().collect();
    let Some(lines) = gitattributes_update(&have) else {
        return Ok(format!("{} already declares the trck drivers", path.display()));
    };
    write_atomic(&path, &(lines.join("\n") + "\n"))?;
    Ok(format!("wrote {}", path.display()))
}

/// The per-clone half: define what the named drivers actually run.
fn register(ctx: &Ctx) -> Result<(), String> {
    let engine = engine_invocation()?;
    let drivers = [
        ("trck-index", format!("{engine} repo merge-index %O %A %B"), "trck index.jsonl row-wise 3-way merge"),
        ("trck-summary", format!("{engine} repo merge-summary %A"), "trck SUMMARY.md regeneration"),
    ];
    for (name, cmd, label) in &drivers {
        git(ctx, &["config", &format!("merge.{name}.driver"), cmd])?;
        git(ctx, &["config", &format!("merge.{name}.name"), label])?;
    }
    Ok(())
}

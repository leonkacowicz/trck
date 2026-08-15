//! `repo install-hook` — a pre-commit hook that runs `trck check` when the tracker changes.

use super::git::{engine_invocation, require_repo};
use crate::discovery::Ctx;
use crate::verbs::write_atomic;
use std::path::{Path, PathBuf};

pub(crate) fn cmd_install_hook(ctx: &Ctx) -> Result<String, String> {
    let hooks = hooks_dir(ctx)?;
    let rel = tracker_rel(ctx)?;
    std::fs::create_dir_all(&hooks).map_err(|e| format!("{}: {e}", hooks.display()))?;
    let hook = hooks.join("pre-commit");
    write_atomic(&hook, &hook_body(&rel, &engine_invocation()?))?;
    make_executable(&hook)?;
    Ok(format!("installed {}", hook.display()))
}

/// Where git looks for hooks — `--git-common-dir`, so a linked worktree installs into the
/// shared hooks directory rather than one of its own that git would never consult.
fn hooks_dir(ctx: &Ctx) -> Result<PathBuf, String> {
    let common = require_repo(ctx, "--git-common-dir")?;
    let hooks = ctx.dir()?.join(&common);
    Ok(hooks.canonicalize().unwrap_or(hooks).join("hooks"))
}

/// The tracker's path relative to the repo root, in git's own forward-slash form — `.` when
/// the tracker dir *is* the root.
fn tracker_rel(ctx: &Ctx) -> Result<String, String> {
    let toplevel = require_repo(ctx, "--show-toplevel")?;
    let root = Path::new(&toplevel).canonicalize().map_err(|e| format!("{toplevel}: {e}"))?;
    let tracker = ctx.dir()?;
    let dir = tracker.canonicalize().map_err(|e| format!("{}: {e}", tracker.display()))?;
    let rel = dir.strip_prefix(&root).map_err(|_| format!("tracker dir {} is not inside the git repo at {}", tracker.display(), root.display()))?;
    Ok(if rel.as_os_str().is_empty() { ".".to_string() } else { rel.to_string_lossy().replace('\\', "/") })
}

/// The hook script.
///
/// It prefers the engine at the absolute path baked in and falls back to whatever `trck` is on
/// `PATH`, so a hook installed from a build tree keeps working after that tree is deleted. When
/// neither is there it does nothing: a machine without trck should still be able to commit.
fn hook_body(rel: &str, engine: &str) -> String {
    let (guard, edir) = staged_guard(rel);
    format!(
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
    )
}

/// When to run the check, and which tracker to point it at.
///
/// When the tracker dir IS the repo root, `rel` is "." and a path-prefix grep would never
/// match git's repo-relative staged paths — the hook would silently never run. There the
/// whole repo is the tracker, so it fires on any staged change.
fn staged_guard(rel: &str) -> (String, String) {
    if rel == "." {
        return ("if [ -n \"$staged\" ]; then".to_string(), "$root".to_string());
    }
    (format!("if printf '%s\\n' \"$staged\" | grep -qE '(^|/){}/'; then", rel.replace('.', "\\.")), format!("$root/{rel}"))
}

/// Mark a file user-executable, which a hook must be for git to run it.
#[cfg(unix)]
fn make_executable(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).map_err(|e| format!("{}: {e}", path.display()))
}

// The `Result` is unnecessary on this platform, and deliberately kept: the two `cfg`
// arms are one function to every caller, and the unix arm can genuinely fail.
#[cfg(not(unix))]
#[allow(clippy::unnecessary_wraps)]
fn make_executable(_path: &Path) -> Result<(), String> {
    Ok(()) // Windows has no executable bit; git runs hooks through the shell there.
}

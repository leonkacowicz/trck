from __future__ import annotations
from pathlib import Path
import subprocess
from .constants import ITEMS_DIR, die
from .diff import Snapshot
from .index import Ctx, Issue, filename, parse_index

# --------------------------------------------------------------------------- #
# diff: the git convenience layer
# --------------------------------------------------------------------------- #
# The ergonomic layer over the source seam: `trck diff` with no arguments, or with
# a bare revision spec, instead of spelling out `--from`. This is the ONLY part of
# `diff` that shells out — it produces the same Snapshot as any other source, so
# nothing downstream behaves differently for having been fed by git.
USE_FROM = "use --from to compare tracker files directly"


def git_run(args: list[str], cwd: Path) -> subprocess.CompletedProcess:
    """Run git, turning "no git at all" into an error that names the way out."""
    try:
        return subprocess.run(["git", *args], cwd=cwd, capture_output=True, text=True)
    except FileNotFoundError:
        die(f"git is not on PATH, so revision specs are unavailable; {USE_FROM}")


def git_tracker_prefix(ctx: Ctx) -> str:
    """The tracker dir as a repo-relative path prefix, as `git show <rev>:<path>`
    wants it. Derived the way `install-hook` derives it, so it works from any
    subdirectory; a tracker dir that IS the repo root yields an empty prefix."""
    r = git_run(["rev-parse", "--show-toplevel"], ctx.dir)
    if r.returncode != 0:
        die(f"not a git repository, so revision specs are unavailable; {USE_FROM}")
    root = Path(r.stdout.strip())
    try:
        rel = ctx.dir.resolve().relative_to(root.resolve()).as_posix()
    except ValueError:
        die(f"tracker dir {ctx.dir} is not inside the git repo at {root}")
    return "" if rel == "." else f"{rel}/"


def git_verify_rev(ctx: Ctx, rev: str) -> None:
    """Fail on a revision git can't resolve — separately from a path that isn't in
    it, so 'you typo'd the branch' and 'the tracker didn't exist yet' stay
    distinguishable."""
    r = git_run(["rev-parse", "--verify", "--quiet", f"{rev}^{{commit}}"], ctx.dir)
    if r.returncode != 0:
        die(f"unknown revision '{rev}'")


def git_show(ctx: Ctx, rev: str, path: str) -> str | None:
    """The blob at `<rev>:<path>`, or None when that path isn't in that revision."""
    r = git_run(["show", f"{rev}:{path}"], ctx.dir)
    return r.stdout if r.returncode == 0 else None


def git_snapshot(ctx: Ctx, rev: str) -> Snapshot:
    """The tracker as of `rev`.

    The tracker dir being absent at that revision is NOT an error — comparing
    against a commit from before the tracker existed is a legitimate question, and
    the answer is that every issue was added since. Bodies are fetched lazily, one
    `git show` per issue, so a run that never asks for one costs nothing.
    """
    prefix = git_tracker_prefix(ctx)
    git_verify_rev(ctx, rev)
    origin = f"{rev}:{prefix}index.jsonl"
    text = git_show(ctx, rev, f"{prefix}index.jsonl")
    rows = parse_index(text, origin) if text is not None else []

    def read_body(row: Issue) -> str | None:
        return git_show(ctx, rev, f"{prefix}{ITEMS_DIR}/{filename(row)}")

    return Snapshot(rows, rev, body_reader=read_body)


def parse_rev_spec(spec: str) -> tuple[str, str | None]:
    """Split a revision spec into (old, new); a None `new` means the working tree.
    `a..b` names both sides; a bare `a` compares that revision to the working tree."""
    if "..." in spec:
        die("three-dot (merge-base) revision specs are not supported; "
            "use `a..b` to compare two revisions directly")
    if ".." not in spec:
        return spec, None
    old, _, new = spec.partition("..")
    if not old or not new:
        die(f"incomplete revision range '{spec}'; both sides of `..` are required")
    return old, new

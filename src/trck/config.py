from __future__ import annotations
from pathlib import Path
import json
import os
from .constants import DEFAULT_UPDATE_REPO, FILENAME_RE, ITEMS_DIR, PR_URL_RE, SELF_PATH, die

# --------------------------------------------------------------------------- #
# config + discovery
# --------------------------------------------------------------------------- #
# `in-review` is a waiting state, not a lifecycle anchor: it carries no role (the
# rollup's one-each initial/active/terminal constraint is untouched) and opts out of
# `actionable`, so ready/next never propose work that is only waiting on a review.
DEFAULT_CONFIG = {
    "update": {"repo": DEFAULT_UPDATE_REPO, "channel": "stable"},
    "statuses": [
        {"name": "backlog", "role": "initial"},
        {"name": "ongoing", "role": "active"},
        {"name": "in-review", "actionable": False},
        {"name": "done", "role": "terminal"},
    ],
    "aliases": {"start": "ongoing", "review": "in-review", "done": "done"},
    "priorities": ["urgent", "high", "medium", "low", "lowest"],
    "default_priority": "medium",
    "kinds": ["task", "epic", "bug", "story", "investigation"],
    "resolutions": ["superseded", "wontfix", "duplicate"],
}


def load_config(tracker_dir: Path) -> dict:
    """Merge trck.json (if any) over DEFAULT_CONFIG. Top-level keys override;
    the nested 'update' dict is merged key-by-key."""
    cfg = json.loads(json.dumps(DEFAULT_CONFIG))  # deep copy
    path = Path(tracker_dir) / "trck.json"
    if path.exists():
        try:
            user = json.loads(path.read_text() or "{}")
        except json.JSONDecodeError as e:
            die(f"{path}: invalid JSON ({e})")
        for k, v in user.items():
            if k == "update" and isinstance(v, dict):
                cfg["update"].update(v)
            else:
                cfg[k] = v
    return cfg


def status_names(cfg: dict) -> list[str]:
    return [s["name"] for s in cfg["statuses"]]


def status_role(cfg: dict, name: str) -> str | None:
    for s in cfg["statuses"]:
        if s["name"] == name:
            return s.get("role")
    return None


def default_priority(cfg: dict) -> str:
    """The priority `trck new` assigns when none is given. An explicit
    `default_priority` wins when it's one of the configured priorities;
    otherwise fall back to the median of the list (a sensible middle, so
    repos that override `priorities` without setting a default still work)."""
    prios = cfg.get("priorities") or []
    dp = cfg.get("default_priority")
    if dp in prios:
        return dp
    return prios[len(prios) // 2] if prios else ""


# --- value-vocabulary checks ------------------------------------------------ #
# One predicate per rule, shared by the command handlers (which `die` on the
# returned message) and `validate` (which appends it to the error list). Each
# returns None when the value is acceptable, else a human-readable message that
# still names the configured options.
def check_priority(cfg: dict, value: str) -> str | None:
    if value in cfg["priorities"]:
        return None
    return f"bad priority '{value}' (configured: {', '.join(cfg['priorities'])})"


def check_kind(cfg: dict, value: str) -> str | None:
    if value in cfg["kinds"]:
        return None
    return f"bad kind '{value}' (configured: {', '.join(cfg['kinds'])})"


def check_resolution(cfg: dict, value: str) -> str | None:
    if value in cfg["resolutions"]:
        return None
    return f"bad resolution '{value}' (configured: {', '.join(cfg['resolutions'])})"


def check_pr(value: str) -> str | None:
    if isinstance(value, str) and PR_URL_RE.match(value):
        return None
    return f"bad pr {value!r} (must be an absolute http(s) URL)"


def check_points(value: int) -> str | None:
    if value >= 0:
        return None
    return f"bad points {value} (must be a non-negative integer)"


def check_status_roles(cfg: dict) -> list[str]:
    """The status vocabulary must name exactly one `initial`, one `active`, and one
    `terminal` status — the three lifecycle anchors the rollup (#67) reasons about.
    Returns one message per role that is missing or duplicated."""
    out = []
    for role in ("initial", "active", "terminal"):
        n = sum(1 for s in cfg["statuses"] if s.get("role") == role)
        if n != 1:
            out.append(f"config: exactly one status must carry role '{role}' (found {n})")
    return out


def check_status_flags(cfg: dict) -> list[str]:
    """`actionable`, when a status declares it, must be a boolean. Returns one
    message per offending status (empty when the vocabulary is well-formed)."""
    return [f"config: status '{s.get('name')}' has a non-boolean 'actionable' "
            f"({s['actionable']!r})"
            for s in cfg["statuses"]
            if "actionable" in s and not isinstance(s["actionable"], bool)]


def detect_legacy_layout(cfg: dict, tracker_dir: Path) -> list[Path]:
    """Issue markdown still sitting in per-status folders — the pre-0.23 layout,
    where the containing directory carried the status. Returns the offending paths
    (sorted, one pass per configured status); empty when the tracker is already
    flat. Only well-formed issue filenames count, so a README or scratch note
    parked in an old folder is not mistaken for an unmigrated issue.

    ITEMS_DIR is skipped: statuses no longer name directories, so a tracker may
    legally configure a status called `items`, and scanning the body directory
    would report every correctly-migrated file as unmigrated."""
    out = []
    for name in status_names(cfg):
        if name == ITEMS_DIR:
            continue
        d = Path(tracker_dir) / name
        if not d.is_dir():
            continue
        out.extend(p for p in sorted(d.glob("*.md")) if FILENAME_RE.match(p.name))
    return out


def is_actionable(cfg: dict, name: str) -> bool:
    """Whether `ready`/`next` may propose an issue in this status as work to pick up.

    True unless the status opts out with `"actionable": false` — the generic way to
    model a waiting state (in-review, qa, awaiting-deploy) where the issue is in
    flight but there is nothing for anyone to start. Unknown statuses fail open."""
    for s in cfg["statuses"]:
        if s["name"] == name:
            return s.get("actionable", True)
    return True


def initial_status(cfg: dict) -> str:
    for s in cfg["statuses"]:
        if s.get("role") == "initial":
            return s["name"]
    return cfg["statuses"][0]["name"]


def active_status(cfg: dict) -> str | None:
    """The status a parent rolls up to while work is in progress (the one status
    carrying `role: active`), or None if the vocabulary declares no active role."""
    for s in cfg["statuses"]:
        if s.get("role") == "active":
            return s["name"]
    return None


def terminal_statuses(cfg: dict) -> list[str]:
    return [s["name"] for s in cfg["statuses"] if s.get("role") == "terminal"]


def is_terminal(cfg: dict, name: str) -> bool:
    return status_role(cfg, name) == "terminal"


def reconcile(cfg: dict, child_statuses: list[str]) -> str | None:
    """The status a parent should carry given its children's statuses (#67):
    all children initial -> initial, all terminal -> terminal, otherwise active
    (any active child, or a partial mix of initial + terminal). Returns None only
    when the vocabulary has no `active` role and the children force the active case;
    a well-formed config (checked by `validate`) never hits that."""
    init = initial_status(cfg)
    if all(s == init for s in child_statuses):
        return init
    if all(is_terminal(cfg, s) for s in child_statuses):
        return terminal_statuses(cfg)[0]
    return active_status(cfg)


def resolve_alias(cfg: dict, verb: str) -> str | None:
    return cfg.get("aliases", {}).get(verb)


def find_tracker(start: Path, required: bool = True) -> Path | None:
    """Walk up from `start` to the folder containing trck.json (or a child holding it).
    With `required=False`, return None instead of dying when nothing is found."""
    start = Path(start).resolve()
    cur = start
    while True:
        if (cur / "trck.json").exists():
            return cur
        hits = sorted(cur.glob("*/trck.json"))
        if len(hits) == 1:
            return hits[0].parent.resolve()
        if len(hits) > 1:
            die(f"ambiguous tracker under {cur} ({len(hits)} found); pass --dir")
        if cur.parent == cur:
            if required:
                die("no tracker found here; run `trck init`")
            return None
        cur = cur.parent


def resolve_tracker_dir(dir_opt, env=None, required: bool = True) -> Path | None:
    """Resolution order: --dir > TRCK_DIR > vendored self-location > walk up from cwd.
    With `required=False`, return None instead of dying when nothing resolves."""
    env = os.environ if env is None else env
    if dir_opt:
        p = Path(dir_opt).resolve()
        if not (p / "trck.json").exists():
            if required:
                die(f"{p} is not a tracker (no trck.json)")
            return None
        return p
    if env.get("TRCK_DIR"):
        return resolve_tracker_dir(env["TRCK_DIR"], env={}, required=required)
    if (SELF_PATH.parent / "trck.json").exists():
        return SELF_PATH.parent
    return find_tracker(Path.cwd(), required=required)


def resolve_tracker_dir_or_die(dir_opt, env=None) -> Path:
    path = resolve_tracker_dir(dir_opt, env=env, required=False)
    if not path:
        env = os.environ if env is None else env
        explicit = dir_opt or env.get("TRCK_DIR")
        if explicit:
            die(f"{Path(explicit).resolve()} is not a tracker (no trck.json)")
        die("no tracker found here; run `trck init`")
    return path


from __future__ import annotations
from pathlib import Path
import json
import os
from .constants import DEFAULT_UPDATE_REPO, FILENAME_RE, ITEMS_DIR, KNOWN_EXTENSIONS, PR_URL_RE, SELF_PATH, SUPPORTED_FORMAT, die

# --------------------------------------------------------------------------- #
# config + discovery
# --------------------------------------------------------------------------- #
# The four statuses. Fixed — not configured, not renameable, not extensible.
#
# They were briefly two vocabularies: canonical `states` with per-tracker display names
# over them. That bought exactly one feature, renaming, and cost a second word for the
# same concept in every message, doc and conversation. Worse, the shipped names were
# gratuitous synonyms of the states they stood for. One vocabulary, four good names.
#
# What each means to the engine — the only three questions it ever asks:
#
#   backlog     not started.                     initial; what `new` assigns.
#   ongoing     started, someone is on it.       what a mixed parent rolls up to.
#   in-review   started, output pending judgement. nothing to pick up.
#   done        finished.                        satisfies a dependency; counts in progress.
#
# `in-review` is the one that needs a rule, because it looks like it overlaps
# `depends_on`:
#
#   depends_on  when the blocker is real work that someone will do and close.
#   in-review   when making it a task would be inventing one.
#
# A code review forces the distinction. The reviewer is not producing a deliverable, they
# are judging yours, so a task for it would be a fiction — and one per reviewable issue
# would double the tracker. The same holds for a vendor reply or a sign-off: nobody here
# will ever close it.
#
# Anything finer than these four — QA versus awaiting-deploy, say — is a custom field.
# Fields already hold one value per key and can declare their allowed values, so a second
# status vocabulary would only overlap them.
BACKLOG, ONGOING, IN_REVIEW, DONE = "backlog", "ongoing", "in-review", "done"
STATUSES = (BACKLOG, ONGOING, IN_REVIEW, DONE)
# verb -> status, for the `start` / `review` / `done` aliases. Constant now that the
# vocabulary is.
VERB_STATUS = {"start": ONGOING, "review": IN_REVIEW, "done": DONE}

# The five priorities, likewise fixed. The plan had allowed per-tracker display names —
# P0..P4 and the like — and that is struck for the same reason it was struck for
# statuses: two words for one concept costs more in every message and conversation than
# renaming buys. The middle one is the default, which is what `new` assigns.
#
# Fixing the count also fixes the shape of the demand vector, which is one slot per
# priority; it used to be sized from config.
PRIORITIES = ("urgent", "high", "medium", "low", "lowest")
DEFAULT_PRIORITY = PRIORITIES[len(PRIORITIES) // 2]

# The three resolutions, fixed with the rest of the vocabulary. A resolution is valid
# only on `done`, and it means *closed without shipping* — the absence of one is the
# normal case, "finished, it went out". That absence is load-bearing: `select_shipped`
# skips any issue carrying a resolution, so this field is the only thing separating a
# changelog entry from a closed issue that produced nothing to announce.
#
# The engine reads the *bit* (set or not); the three names are for the reader:
#
#   superseded  a later issue took over the work.
#   wontfix     decided against; nobody will do it.
#   duplicate   already tracked elsewhere.
#
# Deliberately no `fixed`: it would be the empty-string case spelled out, and setting it
# would silently drop the issue from the changelog it belongs in.
RESOLUTIONS = ("superseded", "wontfix", "duplicate")

# Everything a tracker may still change. The vocabulary keys all left: they were the
# decisions worth making once, for everyone, rather than per repo. What remains is the
# format version (see constants.py) and where `trck update` pulls from.
DEFAULT_CONFIG = {
    "format": SUPPORTED_FORMAT,
    "update": {"repo": DEFAULT_UPDATE_REPO, "channel": "stable"},
}


def check_format(cfg: dict) -> str | None:
    """Whether this engine understands the tracker. Returns None when it does, else a
    die-ready message. Refuses a *newer* format and any extension it does not know;
    an older format is accepted, since that is what the migration verbs are for."""
    fmt = cfg.get("format", SUPPORTED_FORMAT)
    if isinstance(fmt, bool) or not isinstance(fmt, int):
        return f"bad 'format' {fmt!r} in trck.json (must be an integer)"
    if fmt > SUPPORTED_FORMAT:
        return (f"tracker format {fmt} is newer than this engine understands "
                f"(format {SUPPORTED_FORMAT}) — run `trck update`")
    exts = cfg.get("extensions", {})
    if not isinstance(exts, dict):
        return f"bad 'extensions' {exts!r} in trck.json (must be an object)"
    unknown = sorted(k for k in exts if k not in KNOWN_EXTENSIONS)
    if unknown:
        return (f"tracker uses unknown extension(s): {', '.join(unknown)} — run "
                f"`trck update`")
    return None


def load_config(tracker_dir: Path, guard: bool = True) -> dict:
    """Merge trck.json (if any) over DEFAULT_CONFIG. Top-level keys override;
    the nested 'update' dict is merged key-by-key.

    The single choke point for the format guard: every verb builds a Ctx, and every
    Ctx loads a config here. `guard=False` is for `trck update`, which reads the
    config only to find out where to pull from — refusing there would block the one
    verb that fixes a too-new tracker."""
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
    if guard and (m := check_format(cfg)):
        die(m)
    return cfg


def status_names(cfg: dict) -> list[str]:
    """The vocabulary. `cfg` is accepted and ignored: it was configurable once, and
    every caller still threads a config it no longer needs."""
    return list(STATUSES)






def default_priority(cfg: dict) -> str:
    """What `new` assigns when none is given. `cfg` is accepted and ignored, as with
    `status_names` — it was configurable once and every caller still threads one."""
    return DEFAULT_PRIORITY


# --- value-vocabulary checks ------------------------------------------------ #
# One predicate per rule, shared by the command handlers (which `die` on the
# returned message) and `validate` (which appends it to the error list). Each
# returns None when the value is acceptable, else a human-readable message that
# still names the configured options.
def check_priority(cfg: dict, value: str) -> str | None:
    if value in PRIORITIES:
        return None
    return f"bad priority '{value}' (expected one of: {', '.join(PRIORITIES)})"


def check_resolution(cfg: dict, value: str) -> str | None:
    if value in RESOLUTIONS:
        return None
    return f"bad resolution '{value}' (expected one of: {', '.join(RESOLUTIONS)})"


def check_review_url(value: str) -> str | None:
    if isinstance(value, str) and PR_URL_RE.match(value):
        return None
    return f"bad review_url {value!r} (must be an absolute http(s) URL)"


def check_points(value: int) -> str | None:
    if value >= 0:
        return None
    return f"bad points {value} (must be a non-negative integer)"


def check_vestigial_vocabulary(cfg: dict) -> list[str]:
    """Config keys that used to define a vocabulary and no longer do. A tracker still
    carrying one is not broken — the key is ignored — so this is a warning naming the
    replacement, not an error that would lock the tracker out of every verb."""
    gone = {
        "statuses": f"the vocabulary is fixed: {', '.join(STATUSES)}",
        "aliases": f"the verbs map to fixed statuses: {', '.join(STATUSES)}",
        "kinds": "`kind` is an ordinary custom field now (`set --field kind=bug`)",
        "priorities": f"the priorities are fixed: {', '.join(PRIORITIES)}",
        "default_priority": f"the default is fixed: {DEFAULT_PRIORITY}",
        "resolutions": f"the resolutions are fixed: {', '.join(RESOLUTIONS)}",
    }
    return [f"config: '{k}' is no longer configurable and is being ignored ({why})"
            for k, why in gone.items() if k in cfg]


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
    """Whether `ready`/`next` may propose this as work to pick up. False for `in-review`
    — in flight, but its own output is pending someone else\'s judgement, so there is
    nothing here to start — and for `done`."""
    return name not in (IN_REVIEW, DONE)


def initial_status(cfg: dict) -> str:
    return BACKLOG


def active_status(cfg: dict) -> str:
    return ONGOING


def terminal_statuses(cfg: dict) -> list[str]:
    return [DONE]


def is_terminal(cfg: dict, name: str) -> bool:
    return name == DONE


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
    return VERB_STATUS.get(verb)


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


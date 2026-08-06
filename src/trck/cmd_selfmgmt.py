from __future__ import annotations
from pathlib import Path
import json
import os
import shutil
import urllib.error
import urllib.request
from .cmd_maint import _current_version, _refresh_managed_docs, _update_repo
from .constants import SELF_PATH, die
from .net import fetch_url, latest_release, parse_version

# The last thing this engine has to do is explain its own retirement.
#
# `update` fetches `./trck` from the repository *tree* at the newest release tag. The
# cutover deletes that file, so an engine that kept fetching would ask for a path that no
# longer exists and report `HTTP Error 404` — blaming the network for a decision this
# project made. Worse, it would do it forever: there is no later Python release that could
# ever teach it otherwise.
#
# So it stops here, and says where to go. Not by downloading a stub that explains itself:
# the download is accepted if it compiles and mentions `__version__`, so a stub would pass
# validation and replace a working engine with something that cannot run a tracker.
#
# `--ref` is left alone. Someone pinning a specific Python version still can, and that is
# also what makes this safe to state unconditionally rather than probing the network to
# find out whether the line has really ended.
END_OF_LINE = """\
this is the final Python release of trck, and there is nothing left to update to.

trck is a single binary now. Install it with:

  curl -fsSL https://raw.githubusercontent.com/{repo}/main/scripts/install.sh | sh

or from a Homebrew tap, or by downloading a release asset directly. Your tracker needs no
migration — the on-disk format is unchanged, and the binary reads it as it stands.

`trck update --ref <tag>` still fetches a specific Python version, if you need one."""


def cmd_update(args) -> None:
    repo = _update_repo(args)
    cur_ver = _current_version()
    if not args.ref:
        die(END_OF_LINE.format(repo=repo))
    try:
        if args.ref:
            ref = args.ref
            tag, notes = args.ref, ""
        else:
            tag, notes = latest_release(repo)
            ref = tag
            if parse_version(tag) <= parse_version(cur_ver):
                print(f"already up to date (v{cur_ver}; latest {tag})")
                return
        if args.check:
            print(f"update available: {cur_ver} -> {tag}\n{notes}")
            return
        raw = f"https://raw.githubusercontent.com/{repo}/{ref}/trck"
        source = fetch_url(raw)
    except (urllib.error.URLError, urllib.error.HTTPError) as e:
        die(f"update failed (network): {e}")
    except (KeyError, json.JSONDecodeError) as e:
        die(f"update failed (bad response): {e}")

    try:
        compile(source, "<trck-update>", "exec")
    except SyntaxError as e:
        die(f"downloaded trck did not compile; aborting (left current file intact): {e}")
    if "__version__" not in source:
        die("downloaded file does not look like trck (no __version__); aborting, file left intact")

    target = Path(SELF_PATH)
    tmp = target.with_name(target.name + ".trck-update.tmp")
    try:
        tmp.write_text(source)
        try:
            shutil.copymode(target, tmp)
        except OSError:
            pass
        os.replace(tmp, target)
    except OSError as e:
        tmp.unlink(missing_ok=True)
        die(f"update failed (write): {e}")
    print(f"updated {target} to {ref}")
    _refresh_managed_docs(args, source)
    if notes:
        print(f"\n{notes}")



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

def cmd_update(args) -> None:
    repo = _update_repo(args)
    cur_ver = _current_version()
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



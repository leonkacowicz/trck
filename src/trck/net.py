from __future__ import annotations
import json
import re
import urllib.error
import urllib.request

# --------------------------------------------------------------------------- #
# networking seam + version helpers (used by `update`)
# --------------------------------------------------------------------------- #
def fetch_url(url: str, accept: str | None = None) -> str:
    headers = {"User-Agent": "trck"}
    if accept:
        headers["Accept"] = accept
    req = urllib.request.Request(url, headers=headers)
    with urllib.request.urlopen(req, timeout=30) as resp:
        return resp.read().decode("utf-8")


def parse_version(s: str) -> tuple:
    s = s.lstrip("vV").strip()
    parts = []
    for chunk in s.split("."):
        m = re.match(r"\d+", chunk)
        parts.append(int(m.group()) if m else 0)
    return tuple(parts)


def latest_release(repo: str) -> tuple[str, str]:
    url = f"https://api.github.com/repos/{repo}/releases/latest"
    data = json.loads(fetch_url(url, accept="application/vnd.github+json"))
    return data["tag_name"], data.get("body", "")



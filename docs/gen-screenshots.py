#!/usr/bin/env python3
"""Generate terminal-style SVG screenshots of trck running against the example
tracker, for embedding in README.md. Pure standard library — no third-party deps,
matching the engine's own constraint.

Re-run from the repo root whenever command output or the palette changes:

    python3 docs/gen-screenshots.py

For each entry in COMMANDS this invokes ./trck with FORCE_COLOR=1, parses the
ANSI SGR codes trck emits (a small fixed set — see `_ANSI` in ./trck), and lays
the styled text into a rounded "terminal window" SVG under docs/img/. The SVGs
are committed and referenced from README.md by relative path.
"""
import html
import os
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
TRCK = ROOT / "trck"
EXAMPLE = "examples/action-game"
OUT = ROOT / "docs" / "img"

# slug -> argv passed after `trck` (the --dir is added automatically).
COMMANDS = [
    ("tree",       ["tree"]),
    ("deps-graph", ["deps", "--graph"]),
    ("ready",      ["ready"]),
    ("show-21",    ["show", "21"]),
]

# Terminal chrome + a One-Dark-ish mapping of trck's _ANSI names to hex.
BG = "#1b1e24"
TITLE_BG = "#23272e"
FG_DEFAULT = "#c5cdd9"
PALETTE = {  # SGR code -> hex
    31: "#e06c75", 32: "#98c379", 33: "#d19a66", 34: "#61afef",
    35: "#c678dd", 36: "#56b6c2",
    91: "#e88c93", 92: "#b5d99c", 93: "#e6c08a", 94: "#8cc7ff",
    95: "#d9a0e8", 96: "#7fd0db",
}

FS = 13                 # font size (px)
CW = FS * 0.6           # cell width; textLength pins each run to a multiple of it
LH = 14                 # line pitch ≈ 1em so box-drawing verticals tile (no gaps)
PAD = 16                # body padding
TITLE_H = 34            # title-bar height
FONT = ("ui-monospace, 'DejaVu Sans Mono', 'Cascadia Code', "
        "'Menlo', 'Consolas', monospace")

SGR = re.compile("\x1b\\[([0-9;]*)m")


def run(argv):
    env = {**os.environ, "FORCE_COLOR": "1"}
    env.pop("NO_COLOR", None)
    res = subprocess.run(
        [sys.executable, str(TRCK), "--dir", EXAMPLE, *argv],
        cwd=ROOT, env=env, capture_output=True, text=True, check=True,
    )
    return res.stdout


def parse(text):
    """Split ANSI text into lines; each line is a list of (text, fg, bold, dim)
    runs. State carries across the whole stream (trck resets per token, so this
    also tolerates state that spans lines)."""
    lines = []
    fg, bold, dim = None, False, False
    for raw in text.split("\n"):
        runs, pos = [], 0
        for m in SGR.finditer(raw):
            seg = raw[pos:m.start()]
            if seg:
                runs.append((seg, fg, bold, dim))
            for code in (m.group(1) or "0").split(";"):
                code = int(code or 0)
                if code == 0:
                    fg, bold, dim = None, False, False
                elif code == 1:
                    bold = True
                elif code == 2:
                    dim = True
                elif code in PALETTE:
                    fg = PALETTE[code]
            pos = m.end()
        seg = raw[pos:]
        if seg:
            runs.append((seg, fg, bold, dim))
        lines.append(runs)
    if lines and lines[-1] == []:   # drop the trailing newline's empty line
        lines.pop()
    return lines


def esc(s):
    return html.escape(s, quote=False)


def render(argv, lines):
    cols = max((sum(len(t) for t, *_ in line) for line in lines), default=1)
    width = round(PAD * 2 + cols * CW)   # textLength pins line width to cols*CW
    height = round(TITLE_H + PAD + len(lines) * LH + PAD)

    out = [
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" '
        f'height="{height}" viewBox="0 0 {width} {height}" '
        f'font-family="{FONT}" font-size="{FS}">',
        f'<rect width="{width}" height="{height}" rx="8" fill="{BG}"/>',
        f'<path d="M0,8 A8,8 0 0 1 8,0 L{width - 8},0 A8,8 0 0 1 {width},8 '
        f'L{width},{TITLE_H} L0,{TITLE_H} Z" fill="{TITLE_BG}"/>',
    ]
    for i, c in enumerate(("#ec6a5e", "#f4bf4f", "#61c454")):
        out.append(f'<circle cx="{16 + i * 18}" cy="{TITLE_H // 2}" '
                   f'r="6" fill="{c}"/>')
    cmd = "trck " + " ".join(argv)
    out.append(f'<text x="{width // 2}" y="{TITLE_H // 2 + 4}" fill="#8b93a1" '
               f'text-anchor="middle">$ {esc(cmd)}</text>')

    y0 = TITLE_H + PAD + FS
    for li, line in enumerate(lines):
        out.append(f'<text y="{round(y0 + li * LH)}" xml:space="preserve">')
        col = 0
        for text, fg, bold, dim in line:
            n = len(text)
            # Anchor each run at its exact grid column and force its width to
            # n cells. textLength + lengthAdjust="spacing" pins every column,
            # so wide glyphs (●○◐, bars, dashes) can't drift the alignment.
            attrs = [f'x="{round(PAD + col * CW, 2)}"',
                     f'textLength="{round(n * CW, 2)}"',
                     'lengthAdjust="spacing"',
                     f'fill="{fg or FG_DEFAULT}"']
            if bold:
                attrs.append('font-weight="bold"')
            if dim:
                attrs.append('opacity="0.55"')
            out.append(f'<tspan {" ".join(attrs)}>{esc(text)}</tspan>')
            col += n
        out.append('</text>')
    out.append('</svg>')
    return "\n".join(out) + "\n"


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    for slug, argv in COMMANDS:
        svg = render(argv, parse(run(argv)))
        (OUT / f"{slug}.svg").write_text(svg, encoding="utf-8")
        print(f"wrote docs/img/{slug}.svg")


if __name__ == "__main__":
    main()

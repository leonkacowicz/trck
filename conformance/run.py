#!/usr/bin/env python3
"""Run the trck conformance fixtures against a trck binary.

    python3 conformance/run.py                 # against ./trck
    TRCK_BIN=target/release/trck …/run.py      # against another implementation
    python3 conformance/run.py --update        # rewrite the goldens
    python3 conformance/run.py -k gutter       # only fixtures matching a substring

The suite is a *specification*, not the Python engine's tests. It therefore never
imports trck — it execs whatever `TRCK_BIN` points at. That is the whole reason it
exists: a runner that imported the engine would silently become Python-only the moment a
second implementation arrived, and nothing would fail to tell you.

See README.md in this directory for the fixture format.
"""
import argparse
import difflib
import os
import shutil
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
FIXTURES = HERE / "fixtures"
DEFAULT_BIN = HERE.parent / "trck"

# Fixed so anything the engine stamps is reproducible. Every invocation gets the same
# instant; a fixture that needs the clock to move is not expressible yet (see README).
NOW = "2026-01-01T00:00:00Z"

# Output is compared literally, so anything varying between runs has to be replaced
# first. The tracker lives in a temp dir, and `new`/`path`/`which` print paths into it.
TRACKER_PLACEHOLDER = "<TRACKER>"

ARTIFACTS = {                      # golden file -> path inside the tracker
    "expected.index.jsonl": "index.jsonl",
    "expected.SUMMARY.md": "SUMMARY.md",
}


def split_args(line):
    """Split a fixture command line into argv. `shlex` so a quoted title stays one
    argument, `posix=True` so the quotes themselves do not survive into argv."""
    import shlex
    return shlex.split(line, posix=True)


def read_lines(path):
    """Non-empty, non-comment lines of a fixture control file."""
    if not path.is_file():
        return []
    return [ln.strip() for ln in path.read_text().splitlines()
            if ln.strip() and not ln.lstrip().startswith("#")]


def run_trck(binary, tracker, argv, env_extra=None):
    env = dict(os.environ, TRCK_NOW=NOW, NO_COLOR="1")
    env.pop("TRCK_DIR", None)      # the fixture's tracker, never the caller's
    env.update(env_extra or {})
    return subprocess.run([str(binary), "--dir", str(tracker), *argv],
                          capture_output=True, text=True, env=env)


def normalise(text, tracker):
    return text.replace(str(Path(tracker).resolve()), TRACKER_PLACEHOLDER)


def build_tracker(fixture, workdir, binary, env_extra):
    """Create the tracker, apply the fixture's initial state, then its setup commands.
    Returns the tracker path, or raises RuntimeError naming the setup step that failed."""
    tracker = Path(workdir) / "issues"
    tracker.mkdir(parents=True)
    (tracker / "trck.json").write_text("{}\n")
    (tracker / "items").mkdir()

    initial = fixture / "initial"
    if initial.is_dir():
        # Literal on-disk state, for what the verbs cannot produce: a legacy field, an
        # unknown key round-tripping, a deliberately malformed row, a specific config.
        shutil.copytree(initial, tracker, dirs_exist_ok=True)

    for line in read_lines(fixture / "setup"):
        r = run_trck(binary, tracker, split_args(line), env_extra)
        if r.returncode != 0:
            raise RuntimeError(f"setup step failed: {line}\n{r.stderr.strip()}")
    return tracker


def compare(name, golden_path, actual, failures, update):
    """Assert one golden. Absent means *not asserted* — a fixture states what it cares
    about and stays silent on the rest, so adding a column to an unrelated view does not
    churn every fixture.

    `--update` therefore refreshes goldens that exist and does **not** create new ones.
    Creating them would quietly make every fixture assert everything on the first run,
    which is the property this format is built around. To start asserting something,
    `touch expected.index.jsonl` and re-run with `--update`."""
    if update:
        if golden_path.is_file():
            golden_path.write_text(actual)
        return
    if not golden_path.is_file():
        return
    expected = golden_path.read_text()
    if actual != expected:
        diff = "".join(difflib.unified_diff(
            expected.splitlines(keepends=True), actual.splitlines(keepends=True),
            fromfile=f"expected/{name}", tofile=f"actual/{name}"))
        failures.append(diff or f"{name}: differs only in trailing whitespace")


def run_fixture(fixture, binary, update):
    """Returns a list of failure descriptions; empty means the fixture passed."""
    import tempfile
    cmd_lines = read_lines(fixture / "cmd")
    if len(cmd_lines) != 1:
        return [f"cmd: expected exactly one command line, found {len(cmd_lines)}"]

    env_extra = dict(
        kv.split("=", 1) for kv in read_lines(fixture / "env")) or None

    # A brand-new fixture — nothing asserted yet — gets its stdout captured, plus
    # stderr and exit code when they are interesting. Everything beyond that is opted
    # into by creating the file, so `--update` on an existing fixture never widens what
    # it claims.
    fresh = update and not any(fixture.glob("expected.*"))

    failures = []
    with tempfile.TemporaryDirectory() as tmp:
        try:
            tracker = build_tracker(fixture, tmp, binary, env_extra)
        except RuntimeError as e:
            return [str(e)]

        r = run_trck(binary, tracker, split_args(cmd_lines[0]), env_extra)
        out = normalise(r.stdout, tracker)
        err = normalise(r.stderr, tracker)

        if fresh:
            (fixture / "expected.out").write_text(out)
            if err:
                (fixture / "expected.err").write_text(err)
        compare("stdout", fixture / "expected.out", out, failures, update)
        compare("stderr", fixture / "expected.err", err, failures, update)

        # Exit code is the one thing asserted by default: a fixture that forgets to
        # mention it still means "this is supposed to work".
        code_file = fixture / "expected.code"
        want = int(code_file.read_text().strip()) if code_file.is_file() else 0
        if fresh and r.returncode:
            code_file.write_text(f"{r.returncode}\n")
        elif update and code_file.is_file():
            code_file.write_text(f"{r.returncode}\n")
        elif r.returncode != want:
            failures.append(f"exit code: expected {want}, got {r.returncode}\n"
                            f"{err.strip()}")

        for golden, rel in ARTIFACTS.items():
            if not (fixture / golden).is_file():
                continue           # not asserted; `--update` does not opt you in
            path = tracker / rel
            actual = path.read_text() if path.is_file() else ""
            compare(rel, fixture / golden, actual, failures, update)
    return failures


def main(argv=None):
    ap = argparse.ArgumentParser(description="Run the trck conformance fixtures.")
    ap.add_argument("-k", dest="pattern", help="only fixtures whose name contains this")
    ap.add_argument("--update", action="store_true",
                    help="rewrite the goldens from actual output")
    ap.add_argument("--bin", default=os.environ.get("TRCK_BIN") or str(DEFAULT_BIN),
                    help="the trck binary under test (or $TRCK_BIN)")
    args = ap.parse_args(argv)

    binary = Path(args.bin).resolve()
    if not binary.is_file():
        sys.exit(f"error: {binary} not found (set --bin or $TRCK_BIN)")
    if not os.access(binary, os.X_OK):
        sys.exit(f"error: {binary} is not executable")

    names = sorted(p.name for p in FIXTURES.iterdir()
                   if p.is_dir() and (p / "cmd").is_file())
    if args.pattern:
        names = [n for n in names if args.pattern in n]
    if not names:
        sys.exit("error: no fixtures selected")

    failed = 0
    for name in names:
        problems = run_fixture(FIXTURES / name, binary, args.update)
        if problems:
            failed += 1
            print(f"FAIL {name}")
            for p in problems:
                print("\n".join("    " + ln for ln in p.rstrip().splitlines()))
        elif args.update:
            print(f"updated {name}")
        else:
            print(f"ok   {name}")

    verb = "updated" if args.update else "passed"
    print(f"\n{len(names) - failed}/{len(names)} {verb}  ({binary})")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())

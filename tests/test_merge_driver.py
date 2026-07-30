"""The `trck repo merge-index` driver entrypoint (#ex5cugg).

Git hands a merge driver three temp files (%O %A %B) and expects the merged result
written back to %A, exit 0 for resolved and non-zero for conflicted. These tests
drive the handler directly; #2ry5d58 exercises it through real git.
"""
import io
import json
import unittest
from contextlib import redirect_stdout, redirect_stderr
from pathlib import Path
from tempfile import TemporaryDirectory

from tests.helpers import load_trck, make_tracker, ns


def row(iid="abc1234", **over):
    r = {"id": iid, "slug": "alpha", "title": "Alpha", "kind": "task",
         "status": "backlog", "priority": "medium"}
    r.update(over)
    return r


class TestMergeIndexDriver(unittest.TestCase):
    def setUp(self):
        self.t = load_trck()

    def scene(self, tmp, base, a, b):
        """Lay out a tracker plus the three operand files git would pass."""
        d = make_tracker(tmp, {})
        (d / "items").mkdir(exist_ok=True)
        for r in {x["id"]: x for x in (*base, *a, *b)}.values():
            (d / "items" / f"{r['id']}-{r['slug']}.md").write_text(f"# {r['title']}\n")
        paths = {}
        for name, rows in (("base", base), ("a", a), ("b", b)):
            p = Path(tmp) / f"{name}.jsonl"
            p.write_text("".join(json.dumps(r) + "\n" for r in rows))
            paths[name] = str(p)
        return d, paths

    def run_driver(self, d, paths):
        """Returns (exit_code, stderr). Exit 0 means git accepts %A as resolved."""
        err = io.StringIO()
        code = 0
        try:
            with redirect_stdout(io.StringIO()), redirect_stderr(err):
                self.t.cmd_merge_index(ns(dir=str(d), base=paths["base"],
                                          current=paths["a"], other=paths["b"]))
        except SystemExit as e:
            code = e.code or 0
        return code, err.getvalue()

    def merged_lines(self, paths):
        return [ln for ln in Path(paths["a"]).read_text().splitlines() if ln.strip()]

    # --- the clean path ------------------------------------------------------- #

    def test_disjoint_creates_resolve_and_write_both_rows(self):
        with TemporaryDirectory() as tmp:
            d, paths = self.scene(tmp, [], [row("aaa1111")], [row("bbb2222")])
            code, _ = self.run_driver(d, paths)
            self.assertEqual(code, 0)
            ids = sorted(json.loads(l)["id"] for l in self.merged_lines(paths))
            self.assertEqual(ids, ["aaa1111", "bbb2222"])

    def test_result_written_to_A_is_valid_jsonl(self):
        with TemporaryDirectory() as tmp:
            d, paths = self.scene(tmp, [], [row("aaa1111")], [row("bbb2222")])
            self.run_driver(d, paths)
            for line in self.merged_lines(paths):
                json.loads(line)  # raises if the driver wrote anything unparseable

    def test_clean_merge_regenerates_summary_from_the_merged_rows(self):
        """Not from the working-tree index — during a merge that is not yet the
        merged result. This is the ordering fix."""
        with TemporaryDirectory() as tmp:
            d, paths = self.scene(tmp, [], [row("aaa1111", title="From A")],
                                  [row("bbb2222", title="From B")])
            (d / "index.jsonl").write_text("")       # working tree: still empty
            self.run_driver(d, paths)
            summary = (d / "SUMMARY.md").read_text()
            self.assertIn("From A", summary)
            self.assertIn("From B", summary)

    # --- the conflicted path -------------------------------------------------- #

    def test_lifecycle_conflict_exits_nonzero(self):
        with TemporaryDirectory() as tmp:
            d, paths = self.scene(tmp, [row(status="backlog")],
                                  [row(status="ongoing")],
                                  [row(status="done", closed="T1")])
            code, _ = self.run_driver(d, paths)
            self.assertNotEqual(code, 0)

    def test_conflict_is_reported_on_stderr_without_orientation_words(self):
        with TemporaryDirectory() as tmp:
            d, paths = self.scene(tmp, [row(status="backlog")],
                                  [row(status="ongoing")],
                                  [row(status="done", closed="T1")])
            _, err = self.run_driver(d, paths)
            self.assertIn("abc1234", err)
            for word in ("ours", "theirs", "yours"):
                self.assertNotIn(word, err.lower())

    def test_conflicted_output_does_not_parse_as_clean_jsonl(self):
        """A conflicted merge must not leave a plausible-looking file that someone
        `git add`s without reading. It carries markers, so any trck verb fails."""
        with TemporaryDirectory() as tmp:
            d, paths = self.scene(tmp, [row(status="backlog")],
                                  [row(status="ongoing")],
                                  [row(status="done", closed="T1")])
            self.run_driver(d, paths)
            text = Path(paths["a"]).read_text()
            self.assertIn("<<<<<<<", text)
            self.assertIn(">>>>>>>", text)
            with self.assertRaises(json.JSONDecodeError):
                for line in text.splitlines():
                    if line.strip():
                        json.loads(line)

    def test_conflict_leaves_summary_untouched(self):
        """Regenerating from a half-merged index would launder a conflict into a
        plausible rollup. A stale SUMMARY is obvious; a fabricated one is not."""
        with TemporaryDirectory() as tmp:
            d, paths = self.scene(tmp, [row(status="backlog")],
                                  [row(status="ongoing")],
                                  [row(status="done", closed="T1")])
            (d / "SUMMARY.md").write_text("PREVIOUS ROLLUP\n")
            self.run_driver(d, paths)
            self.assertEqual((d / "SUMMARY.md").read_text(), "PREVIOUS ROLLUP\n")

    def test_cleanly_merged_rows_still_appear_alongside_a_conflict(self):
        with TemporaryDirectory() as tmp:
            d, paths = self.scene(
                tmp,
                [row("abc1234", status="backlog"), row("zzz9999")],
                [row("abc1234", status="ongoing"), row("zzz9999")],
                [row("abc1234", status="done", closed="T1"), row("zzz9999")])
            self.run_driver(d, paths)
            self.assertIn("zzz9999", Path(paths["a"]).read_text())

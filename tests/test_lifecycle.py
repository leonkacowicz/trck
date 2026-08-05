import io
import unittest
from contextlib import redirect_stdout
from pathlib import Path
from tempfile import TemporaryDirectory

from tests.helpers import load_trck, make_tracker, ns


class TestLifecycle(unittest.TestCase):
    def setUp(self):
        self.t = load_trck()

    def setup_dir(self, tmp, config=None):
        return make_tracker(tmp, config or {})

    def new(self, d, title="First", **over):
        args = ns(dir=str(d), title=title, priority="high",
                  parent=None, depends=None, spec=None, slug=None)
        for k, v in over.items():
            setattr(args, k, v)
        buf = io.StringIO()
        with redirect_stdout(buf):
            self.t.cmd_new(args)
        return Path(buf.getvalue().strip()).name.split("-")[0]

    def test_new_issue_id_is_string(self):
        with TemporaryDirectory() as tmp:
            d = self.setup_dir(tmp)
            self.new(d)
            ctx = self.t.Ctx(d, self.t.load_config(d))
            rows = self.t.load_index(ctx)
            self.assertTrue(self.t.ID_RE.match(rows[0].id))

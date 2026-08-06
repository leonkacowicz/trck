"""`scripts/install.sh`, driven against a local file:// release.

The installer is the one script that runs before the user has anything, on a machine
nothing is known about. Its header has always said a `file://` base URL is how it gets
tested; until now nothing did, and the gap showed — it required `unzip` to open the
Windows artifact, which a stock Windows does not have, so it downloaded and verified a
file it could not then unpack.

These build a fake release on disk and point the installer at it, so nothing here touches
the network. They build the artifact for *this* machine's target, because the installer
resolves that itself and then asks for exactly one filename — a fixture built for another
one fails at the download, saying nothing useful about the thing under test.

On a Linux runner they cannot prove the claim that matters, since extracting a zip with
`tar` needs bsdtar and Linux has GNU tar. That is why the `installer (windows)` job in CI
runs this same file under Git Bash: there the target *is* Windows, so the ordinary install
test unpacks a real zip with whatever that machine has.
"""
import os
import shutil
import subprocess
import tarfile
import unittest
import zipfile
from hashlib import sha256
from pathlib import Path
from tempfile import TemporaryDirectory

REPO_ROOT = Path(__file__).resolve().parents[2]
INSTALL_SH = REPO_ROOT / "scripts" / "install.sh"
TAG = "v9.9.9"


def make_release(root: Path, target: str, ext: str, payload: str = "#!/bin/sh\necho fake\n"):
    """A release directory laid out the way the installer expects to fetch it."""
    name = f"trck-{TAG}-{target}"
    stage = root / "stage" / name
    stage.mkdir(parents=True)
    binary = "trck.exe" if "windows" in target else "trck"
    (stage / binary).write_text(payload)
    (stage / binary).chmod(0o755)

    assets = root / "assets" / TAG
    assets.mkdir(parents=True)
    archive = assets / f"{name}.{ext}"
    if ext == "zip":
        with zipfile.ZipFile(archive, "w") as z:
            z.write(stage / binary, f"{name}/{binary}")
    else:
        with tarfile.open(archive, "w:gz") as t:
            t.add(stage / binary, f"{name}/{binary}")

    digest = sha256(archive.read_bytes()).hexdigest()
    (assets.parent / TAG / f"{name}.{ext}.sha256").write_text(f"{digest}  {name}.{ext}\n")
    return root / "assets"


def run_install(base: Path, bindir: Path, env_extra=None, path=None):
    env = dict(os.environ)
    env.update(
        TRCK_BASE_URL=base.resolve().as_uri(),
        TRCK_VERSION=TAG,
        TRCK_BIN_DIR=str(bindir),
    )
    if path is not None:
        env["PATH"] = path
    env.update(env_extra or {})
    return subprocess.run(["sh", str(INSTALL_SH)], capture_output=True, text=True, env=env)


@unittest.skipUnless(INSTALL_SH.is_file(), "installer not present")
class TestInstaller(unittest.TestCase):
    def target(self):
        """Whatever this machine's installer will ask for, so the fixture matches.

        Mirrors `detect_target` in the script. It has to: the installer resolves the target
        itself and then asks for exactly one filename, so a fixture built for a different
        one fails at the download with nothing useful to say. That is precisely how this
        first ran on Windows — the fixture was a Linux tarball and the installer wanted a
        Windows zip.
        """
        uname = lambda flag: subprocess.run(
            ["uname", flag], capture_output=True, text=True).stdout.strip()
        machine, system = uname("-m"), uname("-s")
        arch = "x86_64" if machine in ("x86_64", "amd64") else "aarch64"
        if system.startswith(("MINGW", "MSYS", "CYGWIN")):
            return "x86_64-pc-windows-msvc"
        if system == "Darwin":
            return f"{arch}-apple-darwin"
        return f"{arch}-unknown-linux-musl"

    def ext_for(self, target):
        return "zip" if "windows" in target else "tar.gz"

    def test_installs_this_machines_artifact(self):
        with TemporaryDirectory() as tmp:
            root = Path(tmp)
            target = self.target()
            base = make_release(root, target, self.ext_for(target))
            bindir = root / "bin"
            r = run_install(base, bindir)
            self.assertEqual(r.returncode, 0, r.stderr)
            binary = "trck.exe" if "windows" in target else "trck"
            self.assertTrue((bindir / binary).is_file(), r.stdout + r.stderr)
            self.assertTrue(os.access(bindir / binary, os.X_OK))

    def test_a_bad_checksum_aborts_without_installing(self):
        with TemporaryDirectory() as tmp:
            root = Path(tmp)
            target = self.target()
            ext = self.ext_for(target)
            base = make_release(root, target, ext)
            sums = base / TAG / f"trck-{TAG}-{target}.{ext}.sha256"
            sums.write_text("0" * 64 + f"  trck-{TAG}-{target}.{ext}\n")
            bindir = root / "bin"
            r = run_install(base, bindir)
            self.assertNotEqual(r.returncode, 0)
            self.assertIn("checksum", (r.stderr + r.stdout).lower())
            self.assertFalse(any(bindir.glob("trck*")), "installed despite a bad checksum")


@unittest.skipUnless(INSTALL_SH.is_file(), "installer not present")
class TestZipUnpacking(unittest.TestCase):
    """The Windows artifact is a zip, and the installer must open it with what is there.

    Forced through the code path with a fake `uname`, so the zip branch is exercised on any
    machine rather than only on Windows — where it is reached for real.
    """

    TARGET = "x86_64-pc-windows-msvc"

    def run_for_windows(self, tmp, path=None):
        root = Path(tmp)
        base = make_release(root, self.TARGET, "zip")
        bindir = root / "bin"
        # `detect_target` reads uname; override the whole resolution by asking for the
        # archive directly is not possible, so drive the unpack through a real zip and a
        # windows-shaped name by pointing TRCK_BASE_URL at it and faking uname on PATH.
        fake = root / "fakebin"
        fake.mkdir()
        (fake / "uname").write_text('#!/bin/sh\ncase "$1" in -m) echo x86_64;; *) echo MINGW64_NT-10.0;; esac\n')
        (fake / "uname").chmod(0o755)
        env_path = f"{fake}:{path if path is not None else os.environ['PATH']}"
        return run_install(base, bindir, path=env_path), bindir

    def test_a_zip_is_unpacked_with_whatever_is_available(self):
        with TemporaryDirectory() as tmp:
            r, bindir = self.run_for_windows(tmp)
            self.assertEqual(r.returncode, 0, r.stderr)
            self.assertTrue((bindir / "trck.exe").is_file(), r.stdout + r.stderr)

    @unittest.skipIf(shutil.which("unzip") is None, "no unzip to remove")
    @unittest.skipIf(os.name == "nt", "builds a stripped PATH out of symlinks")
    def test_without_unzip_it_says_what_it_needs(self):
        """On this machine `tar` is GNU tar and cannot read a zip, so removing unzip leaves
        nothing that can — which is the case that used to fail with a bare `need unzip`.

        The requirement is not that it succeeds here; it is that it names both ways out
        rather than one, so a Windows user is not told to install a tool they do not need."""
        with TemporaryDirectory() as tmp:
            barren = Path(tmp) / "barren"
            barren.mkdir()
            for tool in ("sh", "uname", "mktemp", "curl", "wget", "tar", "sha256sum",
                         "cut", "tr", "find", "cp", "chmod", "mv", "mkdir", "sed", "head", "rm"):
                src = shutil.which(tool)
                if src:
                    (barren / tool).symlink_to(src)
            r, bindir = self.run_for_windows(tmp, path=str(barren))
            if r.returncode == 0:
                self.skipTest("this machine's tar reads zip; the fallback is not reachable")
            message = (r.stderr + r.stdout).lower()
            self.assertIn("unzip", message, message)
            self.assertIn("tar", message, message)
            self.assertFalse((bindir / "trck.exe").exists())


if __name__ == "__main__":
    unittest.main()

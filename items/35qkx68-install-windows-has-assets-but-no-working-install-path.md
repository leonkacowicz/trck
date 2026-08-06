# install: Windows has assets but no working install path

## Summary
v0.26.0 publishes a correct Windows artifact — `trck-v0.26.0-x86_64-pc-windows-msvc.zip`
holds `trck.exe`, `LICENSE` and `README.md`, and its `.sha256` is a bare hash that
`install.sh` parses exactly as it parses the others. The build is fine. Getting it onto a
Windows machine is the problem.

**`install.sh` under Git Bash / MSYS / Cygwin.** `detect_target` handles `MINGW*`/`MSYS*`/
`CYGWIN*`, so it resolves the right target and the checksum step works (Git for Windows ships
`sha256sum`). Then it hits:

```sh
zip) command -v unzip >/dev/null 2>&1 || die "need unzip"; unzip -q ...
```

Git for Windows does not ship `unzip`. **To be confirmed on a real Windows box** — that is the
first task here, because if it holds, the documented `curl … | sh` fails on Windows at the
last step, having already downloaded and verified the file it cannot open.

Windows 10 1803 and later ship `tar.exe` (bsdtar), which extracts zips perfectly well, so
`tar -xf` is likely the one-line fix, with `unzip` kept as a fallback for older machines.

**Native PowerShell or cmd.** No installer at all. The path is: find the release, download the
zip, extract it, move `trck.exe` somewhere, edit `PATH`. Nothing wrong with it except that
nobody is told any of it.

**Package managers.** Homebrew is macOS and Linux. There is no winget, scoop or chocolatey
packaging, so the ways a Windows user would normally expect to install a CLI are all absent.

**Also missing: aarch64.** The matrix builds `x86_64-pc-windows-msvc` only, so Windows on ARM
is unserved, unlike macOS and Linux which both ship both architectures.

And `choose_bin_dir` prefers `$HOME/.local/bin`, which under Git Bash is a directory Windows
itself has no opinion about and which is not on `PATH` by default. The script does warn.

## Acceptance criteria
- [x] Confirm on a real Windows machine whether `install.sh` under Git Bash gets as far as
      unpacking. Now asserted continuously rather than once: the `installer (windows)` CI job
      runs the installer under Git Bash against a release built on disk and served over
      `file://`, so the day nothing on that machine can open a zip, CI says so.
- [x] Unpacking works with what a stock Windows actually has — `tar -xf` first, `unzip` as the
      fallback rather than the requirement. tar's failure is non-fatal on purpose: the `tar` on
      PATH under Git Bash is GNU tar, which cannot read a zip at all, so it tries and falls
      through rather than trusting a tool it cannot identify in advance.
- [ ] A supported path for someone who does not have Git Bash: either a short PowerShell
      installer alongside `install.sh`, or documented manual steps that name the PATH edit.
- [x] Documented: the README now leads with the install script, names the prebuilt targets, and
      gives Windows both routes (Git Bash, or download the zip and extend `PATH`).
- [ ] Decide whether `aarch64-pc-windows-msvc` joins the matrix, or is a stated non-target.

## Notes
Found by inspecting the published v0.26.0 artifact rather than by installing it — no Windows
machine was involved, which is exactly why the first criterion is to check on one.

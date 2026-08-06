#!/usr/bin/env sh
# trck installer.
#
#   curl -fsSL https://raw.githubusercontent.com/leonkacowicz/trck/main/scripts/install.sh | sh
#
# POSIX sh, not bash: this is the one script that runs before you have anything, so it
# assumes as little as possible about the machine it lands on.
#
# Environment:
#   TRCK_VERSION   tag to install (default: the latest release)
#   TRCK_BIN_DIR   where to put the binary (default: the first writable dir below)
#   TRCK_BASE_URL  where to fetch from — a file:// URL works, which is how this is tested
set -eu

REPO="${TRCK_REPO:-leonkacowicz/trck}"
BASE_URL="${TRCK_BASE_URL:-https://github.com/${REPO}/releases/download}"
API_URL="${TRCK_API_URL:-https://api.github.com/repos/${REPO}/releases/latest}"

say() { printf '%s\n' "$*"; }
die() { printf 'install: %s\n' "$*" >&2; exit 1; }

# One of curl or wget; every mainstream image has one, and Windows 10+ ships curl.
fetch() {
  if command -v curl >/dev/null 2>&1; then curl -fsSL "$1"
  elif command -v wget >/dev/null 2>&1; then wget -qO- "$1"
  else die "need curl or wget"; fi
}
fetch_to() {
  if command -v curl >/dev/null 2>&1; then curl -fsSL -o "$2" "$1"
  elif command -v wget >/dev/null 2>&1; then wget -qO "$2" "$1"
  else die "need curl or wget"; fi
}

# The Rust target triple for this machine. Computed rather than looked up, so the script
# needs no network round-trip just to learn which asset to ask for.
detect_target() {
  os="$(uname -s)"
  arch="$(uname -m)"
  case "$arch" in
    x86_64 | amd64) arch=x86_64 ;;
    aarch64 | arm64) arch=aarch64 ;;
    *) die "unsupported architecture: $arch" ;;
  esac
  case "$os" in
    Linux)
      # musl is the portable choice: statically linked, so it does not care whether this
      # machine's glibc is older than the builder's. Only x86_64 has a gnu build, and
      # even there musl is the safer default.
      printf '%s-unknown-linux-musl' "$arch" ;;
    Darwin) printf '%s-apple-darwin' "$arch" ;;
    MINGW* | MSYS* | CYGWIN*) printf 'x86_64-pc-windows-msvc' ;;
    *) die "unsupported OS: $os" ;;
  esac
}

latest_tag() {
  # Read the tag without a JSON parser: the field is on its own line in GitHub's output,
  # and needing jq to install something is exactly the barrier this is meant to avoid.
  fetch "$API_URL" \
    | sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' \
    | head -n 1
}

# First writable candidate wins, so a user without sudo still gets a working install.
choose_bin_dir() {
  if [ -n "${TRCK_BIN_DIR:-}" ]; then printf '%s' "$TRCK_BIN_DIR"; return; fi
  for d in "$HOME/.local/bin" /usr/local/bin "$HOME/bin"; do
    if [ -d "$d" ] && [ -w "$d" ]; then printf '%s' "$d"; return; fi
  done
  printf '%s' "$HOME/.local/bin"
}

main() {
  target="$(detect_target)"
  tag="${TRCK_VERSION:-$(latest_tag)}"
  [ -n "$tag" ] || die "could not determine the latest release (set TRCK_VERSION)"

  case "$target" in
    *windows*) ext="zip" ;;
    *) ext="tar.gz" ;;
  esac
  name="trck-${tag}-${target}"
  url="${BASE_URL}/${tag}/${name}.${ext}"

  tmp="$(mktemp -d)"
  trap 'rm -rf "$tmp"' EXIT
  say "downloading ${name}.${ext}"
  fetch_to "$url" "$tmp/pkg.${ext}" || die "download failed: $url"

  # Verify when the checksum is published. Absent is not fatal — an older release may
  # predate them — but a mismatch always is.
  if fetch_to "${url}.sha256" "$tmp/pkg.sha256" 2>/dev/null; then
    if command -v sha256sum >/dev/null 2>&1; then
      want="$(cut -d' ' -f1 < "$tmp/pkg.sha256" | tr -d '\r')"
      got="$(sha256sum "$tmp/pkg.${ext}" | cut -d' ' -f1)"
      [ "$want" = "$got" ] || die "checksum mismatch (expected $want, got $got)"
      say "checksum ok"
    fi
  fi

  case "$ext" in
    tar.gz) tar -xzf "$tmp/pkg.tar.gz" -C "$tmp" ;;
    # Zip is the Windows artifact, and Windows is where the fewest tools can be assumed.
    # Windows 10 1803 and later ship bsdtar as tar.exe, which reads zip perfectly well; a
    # stock Windows has no unzip, and Git for Windows does not add one — so requiring unzip
    # meant the installer downloaded and verified a file it then could not open.
    #
    # tar is tried first for that reason, and its failure is not fatal, because the `tar` on
    # PATH is not necessarily that one: under Git Bash it is GNU tar, which cannot read a zip
    # at all. Try, then fall back, and only complain once nothing has worked.
    zip)
      ( cd "$tmp" && tar -xf pkg.zip ) 2>/dev/null \
        || ( command -v unzip >/dev/null 2>&1 && unzip -q "$tmp/pkg.zip" -d "$tmp" ) \
        || die "could not unpack the archive: need a tar that reads zip (Windows 10 1803+) or unzip"
      ;;
  esac

  bin="$(find "$tmp" -type f -name 'trck' -o -type f -name 'trck.exe' | head -n 1)"
  [ -n "$bin" ] || die "archive did not contain a trck binary"

  dir="$(choose_bin_dir)"
  mkdir -p "$dir"
  dest="$dir/$(basename "$bin")"
  # Install to a sibling and rename: replacing a running binary in place fails on some
  # systems, and a half-written one is worse than an old one.
  cp "$bin" "$dest.new"
  chmod 755 "$dest.new"
  mv "$dest.new" "$dest"
  say "installed $dest ($tag)"

  case ":$PATH:" in
    *":$dir:"*) ;;
    *) say "note: $dir is not on your PATH — add it to use \`trck\` directly" ;;
  esac
}

main "$@"

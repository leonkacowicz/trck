# Homebrew formula for trck.
#
# Lives here rather than in a tap so the version bump is one commit in one repo; a tap
# consumes this file. Point one at it with:
#
#   brew tap leonkacowicz/trck https://github.com/leonkacowicz/trck
#   brew install leonkacowicz/trck/trck
#
# The `url`/`sha256` pairs are updated by hand, in the same commit as the `version` — the
# release workflow publishes the `.sha256` files but does not write them back here, so a
# bump that stops at `version` leaves every hash describing the previous release.
class Trck < Formula
  desc "Deterministic in-repo issue tracker"
  homepage "https://github.com/leonkacowicz/trck"
  license "MIT"
  version "0.30.0"

  on_macos do
    on_arm do
      url "https://github.com/leonkacowicz/trck/releases/download/v#{version}/trck-v#{version}-aarch64-apple-darwin.tar.gz"
      sha256 "1216fd3d5521c390a17e48a7372390564735172d46e69234530b2f1800778b31"
    end
    on_intel do
      url "https://github.com/leonkacowicz/trck/releases/download/v#{version}/trck-v#{version}-x86_64-apple-darwin.tar.gz"
      sha256 "9b6b997dedae3c27b7d6be7aec72153cba90c56a78e51485d540b1586e157cf3"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/leonkacowicz/trck/releases/download/v#{version}/trck-v#{version}-aarch64-unknown-linux-musl.tar.gz"
      sha256 "6be09ef8e8d4a7150a5be19a9201be5400cfd6e59ff609c71cc49a82916dccaa"
    end
    on_intel do
      url "https://github.com/leonkacowicz/trck/releases/download/v#{version}/trck-v#{version}-x86_64-unknown-linux-musl.tar.gz"
      sha256 "ffb446ba9bba2a8164836ed1f0bcd0909079f10dd301b5630059fb656b9be0f6"
    end
  end

  def install
    bin.install "trck"
  end

  test do
    # Not a smoke test of `--version`: that would pass on a binary that cannot read a
    # tracker. Build one, then ask a question only a working engine answers.
    (testpath/"issues").mkpath
    (testpath/"issues/items").mkpath
    (testpath/"issues/trck.json").write "{}\n"
    system bin/"trck", "--dir", testpath/"issues", "new", "Hello", "--id", "aaaaaaa"
    assert_match "Hello", shell_output("#{bin}/trck --dir #{testpath}/issues list")
  end
end

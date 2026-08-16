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
  version "0.30.1"

  on_macos do
    on_arm do
      url "https://github.com/leonkacowicz/trck/releases/download/v#{version}/trck-v#{version}-aarch64-apple-darwin.tar.gz"
      sha256 "6e5b39904138cc88af7d8bfc174f9551b63979e99c1091a1ddf02d45ca29314a"
    end
    on_intel do
      url "https://github.com/leonkacowicz/trck/releases/download/v#{version}/trck-v#{version}-x86_64-apple-darwin.tar.gz"
      sha256 "0bd4093adc8b046abbc4117a8e7a90174fed038149f3a85e3eec461e174b0e35"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/leonkacowicz/trck/releases/download/v#{version}/trck-v#{version}-aarch64-unknown-linux-musl.tar.gz"
      sha256 "09ee689f3278065a8fdd1265ff9be9f8a97d5e2741fe478e2908e4803b8c408c"
    end
    on_intel do
      url "https://github.com/leonkacowicz/trck/releases/download/v#{version}/trck-v#{version}-x86_64-unknown-linux-musl.tar.gz"
      sha256 "363527e7cd8c4b6afd3c432af76b7d495c39300ed6901a219770c883c87c5434"
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

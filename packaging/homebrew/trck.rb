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
  version "0.29.1"

  on_macos do
    on_arm do
      url "https://github.com/leonkacowicz/trck/releases/download/v#{version}/trck-v#{version}-aarch64-apple-darwin.tar.gz"
      sha256 "16cc6f05bce409f23c4829ec1e1a4610824e0047bb6ac2dfdd309ff7a4634713"
    end
    on_intel do
      url "https://github.com/leonkacowicz/trck/releases/download/v#{version}/trck-v#{version}-x86_64-apple-darwin.tar.gz"
      sha256 "d6ae56d3ad04194517a07661a446d406add4192cf5b85630f36e7ea247a3c1f8"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/leonkacowicz/trck/releases/download/v#{version}/trck-v#{version}-aarch64-unknown-linux-musl.tar.gz"
      sha256 "8a60fc7596ed42ac381432b5d71f7dee1ca6473e7b78f1ce4907b0227b8a8bb4"
    end
    on_intel do
      url "https://github.com/leonkacowicz/trck/releases/download/v#{version}/trck-v#{version}-x86_64-unknown-linux-musl.tar.gz"
      sha256 "d9fa0ab5064dc7678e2982d810a41cb9084281cd3407ac4a73ec1888ff82baa0"
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

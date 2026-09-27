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
  version "0.31.1"

  on_macos do
    on_arm do
      url "https://github.com/leonkacowicz/trck/releases/download/v#{version}/trck-v#{version}-aarch64-apple-darwin.tar.gz"
      sha256 "1d0d3a7110301b905ef1f9826f6123a187da90aee26c71a27a6cedfb38109a38"
    end
    on_intel do
      url "https://github.com/leonkacowicz/trck/releases/download/v#{version}/trck-v#{version}-x86_64-apple-darwin.tar.gz"
      sha256 "aa3ec88d47eec48047be59a26f1f9234d1392d2b1b6d9e8d05c14ee0cef7dabe"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/leonkacowicz/trck/releases/download/v#{version}/trck-v#{version}-aarch64-unknown-linux-musl.tar.gz"
      sha256 "3626aba0a20e6e70bedadc1749ceb307af62b7a08c43a8c3fefdb662e1606ac3"
    end
    on_intel do
      url "https://github.com/leonkacowicz/trck/releases/download/v#{version}/trck-v#{version}-x86_64-unknown-linux-musl.tar.gz"
      sha256 "1bed69b82ed45d5f55e2aa2b98ab46f474db97de587ce48c0190f7a115eeab9e"
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

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
      sha256 "746970acbfc22a777ee1af8b234a097a2522ea94dbd0398df2dd087f7a9a9332"
    end
    on_intel do
      url "https://github.com/leonkacowicz/trck/releases/download/v#{version}/trck-v#{version}-x86_64-apple-darwin.tar.gz"
      sha256 "41d04e24b148d9346990bf2f5b7d2385e683add877b9dfd59988bb21aec9381b"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/leonkacowicz/trck/releases/download/v#{version}/trck-v#{version}-aarch64-unknown-linux-musl.tar.gz"
      sha256 "03cecfabe0cdfc3d98dbc4424c2404ddd4b22c65f8aa60a18bc94ef266508c30"
    end
    on_intel do
      url "https://github.com/leonkacowicz/trck/releases/download/v#{version}/trck-v#{version}-x86_64-unknown-linux-musl.tar.gz"
      sha256 "2f98dfef352555a8269d8788cd0fa5eb5c48fbb355e7a84011162aae3e8eb1f1"
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

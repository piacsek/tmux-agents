#!/usr/bin/env bash
# Print the Homebrew formula for a released version, fetching the asset hashes
# from the GitHub release. Usage: scripts/homebrew-formula.sh 0.3.0 > Formula/tmux-agents.rb
set -euo pipefail

version="${1:?version without the leading v}"
base="https://github.com/piacsek/tmux-agents/releases/download/v${version}"

sha() {
  curl -fsSL "${base}/tmux-agents-$1.tar.gz.sha256" | cut -d' ' -f1
}

arm_mac="$(sha aarch64-apple-darwin)"
intel_mac="$(sha x86_64-apple-darwin)"
linux="$(sha x86_64-unknown-linux-gnu)"

cat <<EOF
class TmuxAgents < Formula
  desc "Claude Code session picker, status line and blocked alerts for tmux"
  homepage "https://github.com/piacsek/tmux-agents"
  license "MIT"

  depends_on "tmux"

  on_macos do
    on_arm do
      url "${base}/tmux-agents-aarch64-apple-darwin.tar.gz"
      sha256 "${arm_mac}"
    end
    on_intel do
      url "${base}/tmux-agents-x86_64-apple-darwin.tar.gz"
      sha256 "${intel_mac}"
    end
  end

  on_linux do
    on_intel do
      url "${base}/tmux-agents-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "${linux}"
    end
  end

  def install
    bin.install "tmux-agents"
  end

  test do
    assert_match "usage", shell_output("#{bin}/tmux-agents bogus 2>&1", 1)
    assert_match "[preview]", shell_output("#{bin}/tmux-agents config")
  end
end
EOF

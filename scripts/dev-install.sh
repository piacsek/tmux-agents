#!/usr/bin/env bash
# Build the release binary and expose it as `tmux-agents-dev`, separate from the Homebrew `tmux-agents`.
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build --release --locked
mkdir -p ~/.local/bin
ln -sfn "$PWD/target/release/tmux-agents" ~/.local/bin/tmux-agents-dev
echo "tmux-agents-dev -> $PWD/target/release/tmux-agents"

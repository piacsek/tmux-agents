#!/usr/bin/env bash
# Render README screenshots from a throwaway tmux server and a fixture registry.
# Never reads ~/.claude: HOME is a tempdir, the tmux socket is private.
# Usage: scripts/screenshots.sh [out-dir]   (needs tmux, freeze, cargo build --release)
set -euo pipefail

out="${1:-docs}"
root="$(cd "$(dirname "$0")/.." && pwd)"
bin="$root/target/release/tmux-agents"
[ -x "$bin" ] || cargo build --release --manifest-path "$root/Cargo.toml"
mkdir -p "$out"

sock="tmux-agents-shots-$$"
home="$(mktemp -d)"
trap 'tmux -L "$sock" kill-server 2>/dev/null || true; rm -rf "$home"' EXIT
t() { tmux -L "$sock" -f /dev/null "$@"; }

mkdir -p "$home/.claude/sessions" "$home/.config/tmux-agents"
for d in webapp dotfiles blog; do mkdir -p "$home/projects/$d"; done

shot_window="$(t new-session -d -s main -x 140 -y 14 -c "$home" -P -F '#{window_id}')"
t set -g status off

now_ms=$(( $(date +%s) * 1000 ))
agent() {
  local dir="$1" status="$2" age_s="$3" title="$4" waiting="$5" body="$6"
  t new-window -d -t main -n "$dir" -c "$home/projects/$dir" "printf '%b' \"$body\"; sleep 600"
  local pane pid
  pane="$(t display -p -t "main:$dir" '#{pane_id}')"
  pid="$(t display -p -t "main:$dir" '#{pane_pid}')"
  t select-pane -t "$pane" -T "$title"
  local extra=""
  [ -n "$waiting" ] && extra=",\"waitingFor\":\"$waiting\""
  cat >"$home/.claude/sessions/$pid.json" <<EOF
{"pid":$pid,"cwd":"/Users/me/projects/$dir","kind":"interactive","status":"$status",
 "statusUpdatedAt":$(( now_ms - age_s * 1000 ))$extra,
 "tmux":"main:$(t display -p -t "main:$dir" '#{window_id}').$pane"}
EOF
}

prompt='\\033[1m● Bash\\033[0m(gh pr comment 42 --delete-last)\\n\\n\\033[1mAllow this command?\\033[0m\\n  \\033[36m❯ 1. Yes\\033[0m\\n    2. Yes, and don'"'"'t ask again for gh in this session\\n    3. No, and tell Claude what to do differently\\n'
agent webapp   waiting 240  "✳ Remove PR comments"              "permission prompt" "$prompt"
agent dotfiles busy     70  "◐ Tmux Claude Code session picker" ""                  '\\033[2mThinking…\\033[0m\\n'
agent blog     idle  86400  "my-macbook.local"                  ""                  '\\033[2m❯\\033[0m\\n'

cat >"$home/truecolor.py" <<'PY'
import re, sys
PALETTE = {0: (69, 71, 90), 1: (243, 139, 168), 2: (166, 227, 161), 3: (249, 226, 175),
           4: (137, 180, 250), 5: (203, 166, 247), 6: (148, 226, 213), 7: (205, 214, 244)}
FG, DIM_FG = (205, 214, 244), (127, 132, 156)
fg, bold, dim = None, False, False

def style():
    r, g, b = fg if fg else (DIM_FG if dim else FG)
    if dim and fg:
        r, g, b = [(c * 2 + 60) // 3 for c in (r, g, b)]
    return f"\033[0;{'1;' if bold else ''}38;2;{r};{g};{b}m"

def apply(codes):
    global fg, bold, dim
    codes = [int(c) for c in codes.split(';') if c] or [0]
    i = 0
    while i < len(codes):
        c = codes[i]
        if c == 0: fg, bold, dim = None, False, False
        elif c == 1: bold = True
        elif c == 2: dim = True
        elif c == 22: bold = dim = False
        elif c == 39: fg = None
        elif 30 <= c <= 37: fg = PALETTE[c - 30]
        elif 90 <= c <= 97: fg = PALETTE[c - 90]
        elif c == 38 and codes[i + 1] == 5: fg = PALETTE.get(codes[i + 2] % 8); i += 2
        elif c == 38 and codes[i + 1] == 2: fg = tuple(codes[i + 2:i + 5]); i += 4
        i += 1
    return style()

text = sys.stdin.read()
sys.stdout.write(re.sub(r"\033\[([0-9;]*)m", lambda m: apply(m.group(1)), text))
PY

shoot() {
  local name="$1" width="$2" height="$3" config="$4"
  printf '%s' "$config" >"$home/.config/tmux-agents/config.toml"
  t resize-window -t "$shot_window" -x "$width" -y "$height"
  t respawn-pane -k -t "$shot_window" -e "HOME=$home" -e "XDG_CONFIG_HOME=$home/.config" "$bin"
  sleep 1.5
  t capture-pane -e -p -t "$shot_window" | python3 "$home/truecolor.py" >"$home/$name.ansi"
  freeze "$home/$name.ansi" -o "$out/$name.png" --theme catppuccin-mocha \
    --font.size 14 --padding 20 --margin 0 --window
}

shoot picker  90 10 $'[preview]\nenabled = false\n'
shoot preview 140 12 $'[preview]\nenabled = true\n'
echo "wrote $out/picker.png $out/preview.png"

#!/usr/bin/env bash
# Render README screenshots from a throwaway tmux server and a fixture registry.
# Never reads ~/.claude: HOME is a tempdir, the tmux socket is private.
# Usage: scripts/screenshots.sh [out-dir]   (needs tmux, freeze, Google Chrome, cargo build --release)
set -euo pipefail

out="${1:-docs}"
root="$(cd "$(dirname "$0")/.." && pwd)"
bin="$root/target/release/tmux-agents"
[ -x "$bin" ] || cargo build --release --manifest-path "$root/Cargo.toml"
mkdir -p "$out"
font=()
for f in "$HOME/Library/Fonts/FiraCodeNerdFont-Regular.ttf" /usr/share/fonts/truetype/firacode/FiraCodeNerdFont-Regular.ttf; do
  [ -f "$f" ] && font=(--font.file "$f") && break
done

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
PALETTE = [(69, 71, 90), (243, 139, 168), (166, 227, 161), (249, 226, 175),
           (137, 180, 250), (203, 166, 247), (148, 226, 213), (205, 214, 244)]
FG, DIM_FG = (205, 214, 244), (127, 132, 156)
fg, bg, bold, dim = None, None, False, False

def xterm(n):
    if n < 16:
        return PALETTE[n % 8]
    if n < 232:
        n -= 16
        steps = [0, 95, 135, 175, 215, 255]
        return (steps[n // 36], steps[n // 6 % 6], steps[n % 6])
    v = 8 + (n - 232) * 10
    return (v, v, v)

def style():
    r, g, b = fg if fg else (DIM_FG if dim else FG)
    if dim and fg:
        r, g, b = [(c * 2 + 60) // 3 for c in (r, g, b)]
    out = f"\033[0;{'1;' if bold else ''}38;2;{r};{g};{b}m"
    if bg:
        out += "\033[48;2;%d;%d;%dm" % bg
    return out

def color(codes, i):
    if codes[i + 1] == 5:
        return xterm(codes[i + 2]), i + 2
    if codes[i + 1] == 2:
        return tuple(codes[i + 2:i + 5]), i + 4
    return None, i

def apply(text):
    global fg, bg, bold, dim
    codes = [int(c) for c in text.split(';') if c] or [0]
    i = 0
    while i < len(codes):
        c = codes[i]
        if c == 0: fg, bg, bold, dim = None, None, False, False
        elif c == 1: bold = True
        elif c == 2: dim = True
        elif c == 22: bold = dim = False
        elif c == 39: fg = None
        elif c == 49: bg = None
        elif 30 <= c <= 37: fg = PALETTE[c - 30]
        elif 90 <= c <= 97: fg = PALETTE[c - 90]
        elif 40 <= c <= 47: bg = PALETTE[c - 40]
        elif 100 <= c <= 107: bg = PALETTE[c - 100]
        elif c == 38: fg, i = color(codes, i)
        elif c == 48: bg, i = color(codes, i)
        i += 1
    return style()

text = sys.stdin.read()
sys.stdout.write(re.sub(r"\033\[([0-9;]*)m", lambda m: apply(m.group(1)), text))
PY

chrome="/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
[ -x "$chrome" ] || chrome="$(command -v google-chrome || command -v chromium || true)"

render() {
  local name="$1"
  local svg="$home/$name.svg"
  [ -s "$home/$name.ansi" ] || { echo "empty capture for $name" >&2; exit 1; }
  if [ -z "$chrome" ]; then
    freeze "$home/$name.ansi" -o "$out/$name.png" --theme catppuccin-mocha \
      --font.size 14 --padding 20 --margin 0 --window </dev/null
    return
  fi
  freeze "$home/$name.ansi" -o "$svg" --theme catppuccin-mocha "${font[@]}" \
    --font.family "FiraCode Nerd Font" --font.size 14 --padding 20,64,20,20 --margin 0 --window </dev/null
  local w h
  w="$(grep -o 'width="[0-9.]*"' "$svg" | head -1 | grep -o '[0-9]*' | head -1)"
  h="$(grep -o 'height="[0-9.]*"' "$svg" | head -1 | grep -o '[0-9]*' | head -1)"
  "$chrome" --headless=new --disable-gpu --hide-scrollbars --force-device-scale-factor=2 \
    --window-size="$w,$h" --screenshot="$out/$name.png" "file://$svg" >/dev/null 2>&1
}

shoot() {
  local name="$1" width="$2" height="$3" config="$4"
  printf '%s' "$config" >"$home/.config/tmux-agents/config.toml"
  t resize-window -t "$shot_window" -x "$width" -y "$height"
  t respawn-pane -k -t "$shot_window" -e "HOME=$home" -e "XDG_CONFIG_HOME=$home/.config" "$bin"
  sleep 1.5
  t capture-pane -e -p -t "$shot_window" | python3 "$home/truecolor.py" >"$home/$name.ansi"
  render "$name"
}

shoot picker  90 10 $'[preview]\nenabled = false\n'
shoot preview 140 12 $'[preview]\nenabled = true\n'

status_widget="env HOME=$home XDG_CONFIG_HOME=$home/.config $bin status"
t new-session -d -s status -n shell -x 100 -y 3 -c "$home" "clear; sleep 600"
t set -t status status on
t set -t status status-interval 1
t set -t status status-style "bg=#313244,fg=#cdd6f4"
t set -t status status-left " #[bold]demo#[default] "
t set -t status status-left-length 20
t set -t status status-right " #($status_widget) #[fg=#6c7086]│#[default] 12:34 "
t set -t status status-right-length 80
t set -t status window-status-format " #W "
t set -t status window-status-current-format " #W "
t resize-window -t "$shot_window" -x 100 -y 3
t respawn-pane -k -t "$shot_window" -e TMUX= tmux -L "$sock" -f /dev/null attach -t status
sleep 2.5
t capture-pane -e -p -t "$shot_window" | tail -1 | python3 "$home/truecolor.py" >"$home/status.ansi"
t kill-session -t status
render status
echo "wrote $out/picker.png $out/preview.png $out/status.png"

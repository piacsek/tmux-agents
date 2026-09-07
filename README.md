<p align="center">
  <img src="LOGO.png" alt="tmux-agents: all your Claude Code sessions, right in tmux" width="800">
</p>

# tmux-agents

A tmux popup listing the Claude Code sessions in the current server, what each
is doing, and a jump to its pane. Also a status-line segment and a blocked-alert
watcher.

![picker](docs/picker.png)

Keys: `j/k` move, `1-9` focus row, `/` filter, `Enter` focus, `n` new Claude
pane, `x` kill (asks `y/n`), `p` toggle preview, `?` help, `q` quit.

- **Preview**: `p` shows the selected pane's screen on the right (popups ≥ 100
  columns). Off by default; see `[preview]` below.
- **Stale**: a working row unchanged for 30 min reads `stale?`.
- **Status line**: blocked sessions by name, the rest as counts.
- **Watch**: flashes `◉ <session>: <reason>` in every client when a session
  becomes blocked.

![preview](docs/preview.png)

![status line](docs/status.png)

## Install

Homebrew (macOS, Linux), pulls in `tmux` if missing:

```sh
brew install piacsek/tap/tmux-agents
```

Prebuilt binary (macOS arm64/x86_64, Linux x86_64) into `~/.local/bin`:

```sh
mkdir -p ~/.local/bin && curl -fsSL "https://github.com/piacsek/tmux-agents/releases/latest/download/tmux-agents-$(uname -m | sed s/arm64/aarch64/)-$(uname -s | sed 's/Darwin/apple-darwin/;s/Linux/unknown-linux-gnu/').tar.gz" | tar xz -C ~/.local/bin
```

From source (needs Rust 1.98+):

```sh
cargo install --git https://github.com/piacsek/tmux-agents --locked
```

## Set up tmux

Add to `~/.tmux.conf`, then `tmux source-file ~/.tmux.conf`:

```
bind -n M-c display-popup -E -w 70% -h 60% "tmux-agents"
set -g status-right " #(tmux-agents status) | #(tmux-agents cached 5 -- my-slow-widget) "
set-option -g status-interval 1
run-shell -b "tmux-agents watch"
```

`cached <ttl-seconds> -- <command>` reruns a widget at most once per TTL so the
1s interval stays cheap; a failing command is shown but not cached. `watch` runs one instance per server and exits with it.

## Configure

`~/.config/tmux-agents/config.toml` (or `$XDG_CONFIG_HOME`, or
`$TMUX_AGENTS_CONFIG`). Every key is optional; `tmux-agents config` prints the
effective values. Unknown keys are an error.

```toml
[picker]
tick_ms = 500
stale_after_minutes = 30

[preview]
enabled = false        # p toggles at runtime
min_width = 100
split_percent = 50     # list width; preview gets the rest

[status]
prefix = "#[fg=white]󰙴#[default]"   # "" for none
named_blocked = 2      # 0 = count only

[watch]
interval_ms = 1000
quiet_ms = 3000        # per-session re-alert window
display_ms = 4000
skip_active_client = true

[new_pane]
command = "zsh -ic claude"
direction = "horizontal"   # or "vertical"

[labels]               # cwd -> name shown instead of the basename
"~/projects/tmux-agents" = "agents"
```

## Develop

`cargo test`, plus `cargo test -- --ignored` for tests that start a real tmux
server. Sessions come from Claude Code's registry at `~/.claude/sessions/`.
Details and gotchas in `AGENTS.md`.

## License

MIT.

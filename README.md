# tmux-agents

A tmux popup that lists the Claude Code sessions running in the current tmux
server, shows what each one is doing, and jumps to the selected pane. Ships a
`status` subcommand for the tmux status line and a `cached` subcommand that
rate-limits other status-line widgets.

```
◉ blocked  webapp   permission prompt · Remove PR comments   4m
● working  dotfiles    Tmux Claude Code session picker          1m
○ idle     scintilla   ~/projects/scintilla.nvim                3d
```

When the popup is at least 100 columns wide the right half previews the
selected pane (`capture-pane`, refreshed every tick), so a permission prompt or
question can be read without switching to it.

A working row whose status has not changed for 30 minutes reads `stale?`, the
usual sign of a hung session worth killing with `x`.

Keys: `j/k` move, `1-9` focus a row, `/` filter, `Enter` focus pane,
`n` new Claude pane, `x` kill pane (asks `y/n`), `p` toggle preview, `?` help,
`q` quit.

## Install

```sh
cargo install --path . --root ~/.local --locked
```

tmux:

```
bind -n M-c display-popup -E -w 70% -h 60% "tmux-agents"
set -g status-right " #(tmux-agents status) | #(tmux-agents cached 5 -- git-widget) …"
set-option -g status-interval 1
```

`tmux-agents cached <ttl-seconds> -- <command> [args...]` runs the command at
most once per TTL and prints the cached stdout in between, so `status-interval`
can drop to 1s for the live agents segment while expensive widgets keep their
old cadence. Cache files live under `$XDG_CACHE_HOME/tmux-agents/`
(`~/.cache/tmux-agents/` by default), one per distinct argv.

Sessions are read from Claude Code's own registry at
`~/.claude/sessions/*.json`; see `AGENTS.md` for the details and caveats.

## Develop

`cargo test`, plus `cargo test -- --ignored` for the tests that start a real
tmux server. See `AGENTS.md`.

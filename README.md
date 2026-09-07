# tmux-agents

A tmux popup that lists the Claude Code sessions running in the current tmux
server, shows what each one is doing, and jumps to the selected pane. Ships a
`status` subcommand for the tmux status line.

```
◉ blocked  webapp   permission prompt · Remove PR comments   4m
● working  dotfiles    Tmux Claude Code session picker          1m
○ idle     scintilla   ~/projects/scintilla.nvim                3d
```

Keys: `j/k` move, `1-9` focus a row, `/` filter, `Enter` focus pane,
`n` new Claude pane, `?` help, `q` quit.

## Install

```sh
cargo install --path . --root ~/.local --locked
```

tmux:

```
bind -n M-c display-popup -E -w 70% -h 60% "tmux-agents"
set -g status-right " #(tmux-agents status) | …"
set-option -g status-interval 1
```

Sessions are read from Claude Code's own registry at
`~/.claude/sessions/*.json`; see `AGENTS.md` for the details and caveats.

## Develop

`cargo test`, plus `cargo test -- --ignored` for the tests that start a real
tmux server. See `AGENTS.md`.

# AGENTS.md

Notes for agents and humans working on `tmux-agents`. Keep this file short and
factual; verify against the code before treating anything here as live state.

## What this is

A Rust + Ratatui popup for tmux that lists the Claude Code sessions running in
the current tmux server, shows their state, and jumps to the selected pane.
`tmux-agents status` prints the same data as tmux status-line markup.
`PLAN.md` holds the phase history, retros, and the ranked backlog.

## Layout

```
src/main.rs      CLI dispatch, event loop wiring (event::poll → Input::Tick every 500 ms)
src/cli.rs       `tmux-agents` (TUI) | `tmux-agents status`
src/registry.rs  lenient serde of ~/.claude/sessions/<pid>.json
src/tmux.rs      Tmux trait, CliTmux (shells out to `tmux`), pane parsing
src/agents.rs    discover(): join registry × panes × pid liveness → Vec<Agent>, sorted
src/state.rs     Status → State (glyph, word, ratatui style, tmux style)
src/app.rs       App state machine (Mode::{Normal, Filter, Help}), run() loop
src/ui.rs        rendering; KEYS table drives the help view
src/status.rs    status-line renderer
tests/           outside-in tests drive run() with a TestBackend + FakeTmux
```

## Data source (undocumented, internal to Claude Code)

`~/.claude/sessions/<pid>.json`, one per live process, removed on exit, with
`.key` sidecars alongside. Fields used: `pid`, `cwd`, `kind`
(`interactive|bg|daemon|daemon-worker`), `status` (`busy|shell|idle|waiting`),
`statusUpdatedAt`, `waitingFor` (only when waiting), `tmux`
(`<session>:@<window>.%<pane>`, absent outside tmux). Honors `CLAUDE_CONFIG_DIR`.

- Parse leniently: unknown fields ignored, unknown enum values → `Unknown`.
- Filter by `kill(pid, 0)` (EPERM counts as alive) and by pane existence.
- `tests/registry.rs` carries a verbatim sample; refresh it when the format changes.
- `waiting` is set for permission dialogs, AskUserQuestion/elicitation, sandbox
  and worker requests, and local command dialogs. `shell` has no known writer;
  it maps to working.
- `claude agents --json` does not exist (checked on 2.1.263); there is no
  official enumeration API.

## Behaviour that is easy to break

- **Draw before the first read.** `run()` renders, then waits for input. The
  popup was blank until a keypress once; `tests/e2e.rs` guards it by running the
  real binary in a scratch tmux server and capturing the pane.
- **Row order is stable while open.** `App::refresh` merges by pid: existing rows
  keep their position, new ones append, gone ones drop. Initial sort is blocked,
  working, idle. Selection follows the pid.
- **Focus from a popup.** `tmux switch-client -Z -t %<pane>` without a client
  target resolves to the client behind the popup; one command switches session,
  window and pane. `command-prompt` does not work from a popup (the prompt shows,
  its command dies with the popup client), which is why `n` uses `split-window`.
- **Title stripping** removes any leading non-alphanumeric glyph plus space
  (Claude uses `✳` and spinner glyphs); a plain hostname title becomes `None`
  and the row falls back to `~/cwd`.
- **Status-line output is tmux markup**, never ANSI. Zero sessions prints
  `none` so a config-level separator never dangles.
- **Colors** come from the ANSI palette (idle is `dim` with no fg) so terminal
  themes apply. Do not hardcode hex.

## Development

Strict TDD: one failing test, minimal code, refactor. Outside-in: start in
`tests/picker.rs` (drives `run()` with scripted `Input`s), drop to unit tests
only for pure helpers. No code comments; names and tests carry the intent.

Gates, all required before a task is done:

```
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo test -- --ignored        # needs a tmux binary; spawns `tmux -L` servers
```

Layout tests are `insta` snapshots in `tests/snapshots.rs`. After an
intentional layout change run `INSTA_UPDATE=always cargo test --test snapshots`,
read the `.snap` diff, and commit it. Behaviour tests assert on row prefixes
only, so layout changes do not ripple through them.

Test-writing traps hit so far: `screen.contains("b")` matched "keybindings";
cell coordinates are 0-based and rows start after the 2-cell highlight symbol;
right-aligned columns make every row the same length.

Install: `cargo install --path . --root ~/.local --locked`. Reinstall after
every change or the popup keeps running the old binary. `tmux display -p`
cannot evaluate `#()`; verify status-line output with `tmux run-shell` instead.

## tmux integration (lives in the user's dotfiles, not here)

```
bind -n M-c display-popup -E -w 70% -h 60% "tmux-agents"
set -g status-right " #(tmux-agents status) #[fg=white]|#[default] …"
set-option -g status-interval 1
```

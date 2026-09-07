# AGENTS.md

Notes for agents and humans working on `tmux-agents`. Keep this file short and
factual; verify against the code before treating anything here as live state.

## What this is

A Rust + Ratatui popup for tmux that lists the Claude Code sessions running in
the current tmux server, shows their state, and jumps to the selected pane.
`tmux-agents status` prints the same data as tmux status-line markup.
`tmux-agents cached <ttl> -- <cmd>` is a status-line helper unrelated to
agents: it runs `<cmd>` at most once per TTL and serves the cached stdout.
`PLAN.md` holds the phase history, retros, and the ranked backlog.

## Layout

```
src/main.rs      CLI dispatch, event loop wiring (event::poll → Input::Tick every 500 ms)
src/cli.rs       `tmux-agents` (TUI) | `tmux-agents status` | `tmux-agents cached <ttl> -- <cmd>`
src/cached.rs    TTL cache for status-line widgets; key = hash of argv, freshness = file mtime
src/registry.rs  lenient serde of ~/.claude/sessions/<pid>.json
src/tmux.rs      Tmux trait (list_panes, focus, new_claude_pane, kill_pane, capture), CliTmux, pane parsing
src/agents.rs    discover(): join registry × panes × pid liveness → Vec<Agent>, sorted
src/state.rs     Status → State (glyph, word, ratatui style, tmux style)
src/app.rs       App state machine (Mode::{Normal, Filter, Help, Confirm}), run() loop
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
- **`x` is the only destructive key** and always goes through
  `Mode::Confirm(pane)`; only `y` yields `Action::Kill`, any other key cancels.
  After `kill-pane` the popup stays open and refreshes so the row disappears.
- **Preview** is `capture-pane -p` of the selected pane, fetched once per
  `run()` iteration (every key and tick), shown only when the body is at least
  `PREVIEW_MIN_WIDTH` (100) columns so the 60-column test terminal never
  previews. Trailing blank lines are dropped and only the last `height` lines
  are drawn. A failed capture (pane vanished mid-tick) means no preview, never
  an exit. `p` toggles it for the life of the popup; the choice is not
  persisted.
- **Stale rows**: a working agent whose `statusUpdatedAt` is older than
  `agents::STALE_AFTER` (30 min) renders the word `stale?` with a dimmed glyph.
  It still sorts and counts as working; the registry has no hung-process signal,
  so this is a hint, not a state.
- **Title stripping** removes any leading non-alphanumeric glyph plus space
  (Claude uses `✳` and spinner glyphs); a plain hostname title becomes `None`
  and the row falls back to `~/cwd`.
- **Status-line output is tmux markup**, never ANSI. Zero sessions prints
  `none` so a config-level separator never dangles. The blocked segment names
  the sessions (`◉ webapp dotfiles +1`, two names then `+n`, oldest prompt
  first) while working and idle stay counts.
- **`cached` stamps the cache file's mtime with the caller's `now`** and reads
  freshness from that mtime, so tests drive it with a fixed clock and a
  tempdir. Writes go to a `.tmp<pid>` sibling then `rename`, so a status-line
  tick never reads a half-written file. Trailing newlines are trimmed and
  stderr is discarded, matching the old `scripts/tmux-cached` shell script.
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

Help-view tests use 12-row terminals (`Picker::with_size`) because the KEYS
table no longer fits the default 60×8; preview tests use 120 columns.

Layout tests are `insta` snapshots in `tests/snapshots.rs`. After an
intentional layout change run `INSTA_UPDATE=always cargo test --test snapshots`,
read the `.snap` diff, and commit it. Behaviour tests assert on row prefixes
only, so layout changes do not ripple through them.

Test-writing traps hit so far: `screen.contains("b")` matched "keybindings";
cell coordinates are 0-based and rows start after the 2-cell highlight symbol;
right-aligned columns make every row the same length.

Install: `cargo install --path . --root ~/.local --locked`. Reinstall after
every change or the popup keeps running the old binary. A milestone (backlog
item, phase) is done only after the gates pass **and** the local binary has
been reinstalled, so the popup and status line the user sees always run the
committed code. `tmux display -p`
cannot evaluate `#()`; verify status-line output with `tmux run-shell` instead.

## tmux integration (lives in the user's dotfiles, not here)

```
bind -n M-c display-popup -E -w 70% -h 60% "tmux-agents"
set -g status-right " #(tmux-agents status) #[fg=white]|#[default] #(tmux-agents cached 5 -- ~/dotfiles/scripts/tmux-git-widget '#{pane_current_path}') …"
set-option -g status-interval 1
```

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
src/cli.rs       `tmux-agents` (TUI) | `status` | `watch` | `config` | `cached <ttl> -- <cmd>` | `--help` | `--version`
src/config.rs    Config (serde + toml), defaults, XDG path resolution, `label_map`
scripts/homebrew-formula.sh   prints the tap formula for a released version
scripts/screenshots.sh        renders docs/*.png from a private tmux server + fixture registry
src/watch.rs     Watcher (pure transition detector) + run() loop + pid lock for `watch`
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
- Filter by `kill(pid, 0)` and by pane existence. EPERM counts as dead: a pid we
  cannot signal belongs to another user, so it is neither a Claude session of
  ours nor a `watch` holder (a recycled pid must not block `watch` forever).
- `tests/registry.rs` carries a verbatim sample; refresh it when the format changes.
- `waiting` is set for permission dialogs, AskUserQuestion/elicitation, sandbox
  and worker requests, and local command dialogs. `shell` has no known writer;
  it maps to working.
- `claude agents --json` does not exist (checked on 2.1.263); there is no
  official enumeration API.

## Behaviour that is easy to break

- **`--help`/`-h`/`help` and `--version`/`-V`/`version`** print to stdout and
  exit 0 before the config is read, so a broken config never hides them.
- **Config is loaded before any subcommand runs.** Missing file = `Config::default()`;
  an unreadable file or unknown key is a hard error naming the file (`deny_unknown_fields`
  on every table). `tmux-agents config` prints the effective TOML and doubles as the
  reference. Constants that used to live in code (tick, stale threshold, preview
  width/split, status prefix, watch timings, new-pane command, labels) now come from
  `App::config`, `CliTmux::with_new_pane`, `status::render(_, &StatusLine)`,
  `watch::run(_, _, _, &Watch)` and `discover(_, _, _, _, &labels)`. Preview is **off**
  by default; tests that need it pass a `Config` with `preview.enabled = true` via
  `Picker::with_config`, and the e2e preview test sets `TMUX_AGENTS_CONFIG`.
- **Draw before the first read.** `run()` renders, then waits for input. The
  popup was blank until a keypress once; `tests/e2e.rs` guards it by running the
  real binary in a scratch tmux server and capturing the pane.
- **Duplicate labels get `:<window_index>`** in `agents::disambiguate`, so two
  checkouts named `webapp` read `webapp:1` and `webapp:5` in the list, the
  kill prompt and the status line. Two in the same window fall back to the
  pane id (`webapp:%1`). Unique labels are untouched.
- **Filter feedback.** The footer reads `/query  matches/total` while
  filtering; zero matches replace the body with `no matches for /query`
  (the "no sessions" view is only for an empty registry).
- **Rows are numbered 1-9** in a dim column after the highlight, matching the
  digit keys; the tenth row onwards shows a blank slot. Numbers follow the
  visible (filtered) list, exactly like the keys do.
- **Row order is stable while open.** `App::refresh` merges by pid: existing rows
  keep their position, new ones append, gone ones drop. Initial sort is blocked,
  working, idle. Selection follows the pid.
- **Focus from a popup.** `tmux switch-client -Z -t %<pane>` without a client
  target resolves to the client behind the popup; one command switches session,
  window and pane. `n` runs `new_pane.command` through tmux's `sh -c`; the
  default `"${SHELL:-sh}" -ic claude` picks up the user's login shell and rc
  files instead of assuming zsh. `command-prompt` does not work from a popup (the prompt shows,
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
- **`watch` alerts only on transitions.** `Watcher::observe` treats its first
  call as a baseline (no alerts for sessions already blocked at startup), then
  alerts when a pid is blocked now and was not blocked before, subject to a
  3s per-pid quiet window (`watch::QUIET`). Alerts go to every client from
  `list-clients` whose active pane is not the blocked pane, via
  `display-message -d 4000 -c <client>`. The loop returns the first tmux error,
  which is how it dies with the server. `watch::claim` writes a pid lock at
  `~/.cache/tmux-agents/watch-<hash of $TMUX socket>.pid`; a live holder makes
  a second `watch` print `watch already running (pid N)` to stderr and exit 0,
  so re-sourcing the config is safe. `Claim::Acquired` carries a `Lock` guard
  that removes the file when `run` returns, so a dead server leaves no lock.
- **`cached` stamps the cache file's mtime with the caller's `now`** and reads
  freshness from that mtime, so tests drive it with a fixed clock and a
  tempdir. Writes go to a `.tmp<pid>` sibling then `rename`, so a status-line
  tick never reads a half-written file. Trailing newlines are trimmed and
  stderr is discarded, matching the old `scripts/tmux-cached` shell script. A
  non-zero exit is printed but not stored, so a transient failure is retried on
  the next tick instead of being served for a whole TTL. `cached::private_dir`
  creates `~/.cache/tmux-agents` as 0700 (and re-tightens an existing dir);
  `watch::claim` uses the same helper for the lock.
- **Missing tmux binary.** `CliTmux::run` maps the spawn `NotFound` to
  `tmux not found on PATH; install tmux (brew install tmux)`; the TUI, `status`
  and `watch` exit 1 with that line, `config` and `cached` never touch tmux.
  `tests/missing_tmux.rs` runs the real binary with an empty `PATH`. The tap
  formula declares `depends_on "tmux"`.
- **Text handed to tmux is format-escaped.** `display-message` and `#()` status
  output both expand `#{…}` and `#[…]` (verified on tmux 3.7c; `#()` is not run).
  Labels and `waitingFor` come from directory names and Claude Code, so
  `tmux::escape` doubles every `#` in `watch` alerts and `status` names.
- **Colors** come from the ANSI palette (idle is `dim` with no fg) so terminal
  themes apply. Do not hardcode hex.

## Documentation rule

`README.md` embeds `docs/picker.png`, `docs/preview.png` and `docs/status.png`.
After any visible layout change run `scripts/screenshots.sh` (needs `brew
install charmbracelet/tap/freeze`, Google Chrome, and the FiraCode Nerd Font in
`~/Library/Fonts`) and commit the new images. Freeze only lays out an SVG with
the font embedded; headless Chrome rasterises it, because freeze's own PNG
output goes blank with any non-default font and cannot draw Nerd Font glyphs.
The status-line shot attaches an inner tmux client to a second session inside a
pane and captures that pane's last row, since `capture-pane` never includes the
status bar. The script never reads
`~/.claude`: it uses a tempdir `HOME`, a private tmux socket (`-L`, `-f
/dev/null`) and invented session names, so real sessions cannot leak into the
README. Do not screenshot a live server.

Whenever behaviour changes, check that `README.md` still covers it and is still
true: a key, a default, a subcommand, a config key. The README must stay terse:
one line per feature, defaults in the TOML block, no prose that repeats the code.
Depth belongs here in `AGENTS.md`, not in the README.

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
cell coordinates are 0-based and rows start after the 2-cell highlight symbol
plus the 2-cell row number (the glyph sits at x=4, the state word at x=6);
right-aligned columns make every row the same length.

Install: `cargo install --path . --root ~/.local --locked`. Reinstall after
every change or the popup keeps running the old binary. A milestone (backlog
item, phase) is done only after the gates pass **and** the local binary has
been reinstalled, so the popup and status line the user sees always run the
committed code. `tmux display -p`
cannot evaluate `#()`; verify status-line output with `tmux run-shell` instead.

## Releasing

Releases are GitHub Releases built by `.github/workflows/release.yml` on a `v*` tag:
one flat tarball per target (`tmux-agents-<target>.tar.gz` holding just the
binary; targets `aarch64-apple-darwin`, `x86_64-apple-darwin`,
`x86_64-unknown-linux-gnu`) plus `.sha256`, with generated notes. Asset names
carry no version so the README's `releases/latest/download/` one-liner stays
valid; do not rename them. Both macOS targets build on `macos-latest` (the Intel
one cross-compiled); the `macos-13` runner label is retired and a job asking for
it queues forever. The workflow
refuses a tag whose version differs from `Cargo.toml`.

1. Bump `version` in `Cargo.toml` (`Cargo.lock` follows on the next build) and
   refresh the help snapshot: `INSTA_UPDATE=always cargo test --test snapshots`.
2. Run the gates, commit `Release vX.Y.Z`, push `main`.
3. `git tag vX.Y.Z && git push origin vX.Y.Z`, then `gh run watch` until the
   `release` workflow is green and `gh release view vX.Y.Z` lists three tarballs.
4. The `tap` job then regenerates `Formula/tmux-agents.rb` in
   `github.com/piacsek/homebrew-tap` (local clone: `~/projects/homebrew-tap`) with
   `scripts/homebrew-formula.sh <version>` and pushes with the `TAP_TOKEN` repo
   secret (a fine-grained PAT with contents:write on homebrew-tap). If that job
   fails, run the script by hand and push the tap. `brew install
   piacsek/tap/tmux-agents` must keep working; check with `brew audit --strict
   --online --formula piacsek/tap/tmux-agents` after changing the script.

Installed copies: Homebrew puts the release binary in `$(brew --prefix)/bin`,
`cargo install --path .` puts dev builds in `~/.local/bin`. Keep only one on
`PATH` while developing or the popup may run the wrong one.

Semver: minor for new keys/subcommands/config, patch for fixes, major on a
config-format break. Local installs keep using `cargo install --path .`.

## tmux integration (lives in the user's dotfiles, not here)

```
bind -n M-c display-popup -E -w 70% -h 60% "tmux-agents"
set -g status-right " #(tmux-agents status) #[fg=white]|#[default] #(tmux-agents cached 5 -- ~/dotfiles/scripts/tmux-git-widget '#{pane_current_path}') …"
set-option -g status-interval 1
run-shell -b "tmux-agents watch"
```

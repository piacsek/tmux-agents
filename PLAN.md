# tmux-agents: herdr-style Claude Code agent picker for tmux

## Context

You trialed herdr (reverted in `6292e95`) and want its one good idea inside tmux: a popup listing every running coding agent, with state, that jumps to the agent's pane. Three phases; Phases 2 and 3 get re-planned after a retrospective on the phase before. Rust + Ratatui, strict `/tdd`, CI in this repo.

Decisions taken (your answers):
- Data source: `~/.claude/sessions/<pid>.json` registry.
- Row label: basename of the session `cwd`, disambiguated by window index on collision.
- Cadence: run all Phase 1 TDD cycles, pause only for the end-of-phase retrospective.

## Verified facts driving the design

**Registry** (`~/.claude/sessions/<pid>.json`, one per live `claude` process, deleted on exit). Example:
```json
{"pid":27756,"sessionId":"dd9b…","cwd":"/Users/me/dotfiles","kind":"interactive","entrypoint":"cli",
 "tmux":"dotfiles:@7.%53","name":"dotfiles-d8","status":"busy","statusUpdatedAt":1788804089018,"version":"2.1.263"}
```
- `status` enum (pulled from the 2.1.263 binary): `busy | shell | idle | waiting`. Claude Code's own agents view maps `busy|shell → working`, `waiting → blocked`, `idle → idle`.
- `kind` enum: `interactive | bg | daemon | daemon-worker`. Only `interactive` has a pane.
- `tmux` = `<session>:@<window_id>.%<pane_id>`; absent outside tmux.
- Format is internal and undocumented. Parse leniently (unknown fields ignored, unknown enum → `Unknown`), filter stale entries by `kill(pid,0)` and by pane existence.
- `claude agents --json` does not exist in 2.1.263 (`unknown option`), so no official enumeration.

**tmux** (3.7b, `.tmux.conf`):
- Popup bindings pattern at `.tmux.conf:63-72`; free alt keys include `M-c`.
- Commands run inside a `display-popup` without `-t` target the client behind the popup, so `switch-client -t %<pane>` then `select-window`/`select-pane` focuses the caller's view. Popup must exit (`-E`) before the switch is visible.
- `status-right` at `.tmux.conf:119` AND in untracked `~/.tmux_work.conf` on this machine, which fully replaces it. Phase 3 must edit both.
- `status-interval 5` today; Phase 3 wants 1.
- Status refresh from a background process: `tmux refresh-client -S -t <client_tty>` per client (pattern in `scripts/tmux-git-flow:14-20`).

**Repo**:
- No CI, no Rust. Rust 1.98.1 via asdf (`.tool-versions`). `.gitignore` is only `*.log`.
- fswatch daemon auto-commits every save. `target/` must be gitignored before the first `cargo build`. Expect many small commits during TDD; that is accepted.
- PATH includes `~/dotfiles/scripts/` and `~/.local/bin`; tmux inherits it.
- `~/.claude/settings.json` → `dotfiles/claude-settings.json` (tracked). Hooks for Phase 2/3 can live there.
- `/tdd` skill at `.claude/skills/tdd/SKILL.md`: outside-in, one failing test at a time, refactor mandatory, no code comments, never commit (auto-sync does it).

---

## Phase 1 — barebones picker

### Crate layout (`~/dotfiles/tmux-agents/`)

```
Cargo.toml, Cargo.lock (committed)
src/main.rs        glue only: cli::parse → registry::load → discover → ratatui::run
src/lib.rs         pub mod cli, registry, tmux, agents, app, ui, process
src/cli.rs         Command::{Tui}; hand-rolled parse (Phase 3 adds Status)
src/registry.rs    SessionRecord, Status, Kind; load(dir); sessions_dir(config_dir, home)
src/tmux.rs        Tmux trait, PaneId, PaneInfo, parse_pane_ref, parse_list_panes, CliTmux
src/agents.rs      Agent; discover(records, panes, alive) → Vec<Agent>  (pure join/filter)
src/app.rs         App (selection), Action, handle_key, run<B: Backend, T: Tmux>
src/ui.rs          draw(frame, &mut App)
src/process.rs     is_alive(pid) via nix kill(pid, None); EPERM counts as alive
tests/support/mod.rs   FakeTmux (records focus calls), fixture builders
tests/picker.rs        outside-in: run() + TestBackend + scripted keys + FakeTmux
tests/discover.rs      pure discovery rules
tests/registry.rs      tempfile fixtures, incl. a verbatim copy of a real session file
tests/tmux_live.rs     #[ignore]; real `tmux -L tmux-agents-test` server, CI only
```

Lib + bin split so `tests/` can drive everything; `main.rs` stays untested glue.

### Key types

```rust
// registry.rs — lenient serde, unknown → Unknown
enum Status { Busy, Shell, Idle, Waiting, #[serde(other)] Unknown }
enum Kind   { Interactive, Bg, Daemon, DaemonWorker, #[serde(other)] Unknown }
struct SessionRecord { pid: i32, cwd: PathBuf, name: Option<String>,
                       kind: Kind, status: Status, tmux: Option<String> }
fn load(dir: &Path) -> Vec<SessionRecord>           // *.json only; skip .key + malformed
fn sessions_dir(config_dir: Option<PathBuf>, home: &Path) -> PathBuf  // $CLAUDE_CONFIG_DIR or ~/.claude

// tmux.rs
struct PaneId(String);                              // "%53"
struct PaneInfo { id, session, window_id, window_index, current_path, title }
trait Tmux { fn list_panes(&self) -> io::Result<Vec<PaneInfo>>;
             fn focus(&self, pane: &PaneId) -> io::Result<()>; }
fn parse_pane_ref("dotfiles:@7.%53") -> Option<PaneId>
fn parse_list_panes(stdout) -> Vec<PaneInfo>       // tab-separated, title last, splitn
struct CliTmux { socket_name: Option<String> }      // -L for the live test
// focus argv = ["switch-client", "-Z", "-t", "%53"]  (tmux 3.7 accepts a pane target: switches session+window+pane in one call)

// agents.rs
struct Agent { pid, label: String, cwd, status, pane: PaneId, session, window_index, title: Option<String> }
fn discover(records, panes: &[PaneInfo], alive: &dyn Fn(i32) -> bool) -> Vec<Agent>
// label = basename(cwd); on collision within the list append " ·<session>:<window_index>"

// app.rs
enum Action { Continue, Quit, Focus(PaneId) }
struct App { agents: Vec<Agent>, list: ListState }
fn run(terminal, app, events: impl Iterator<Item = io::Result<Event>>, tmux: &impl Tmux) -> io::Result<()>
// the only side effect (focus) lives in run(); App is pure state
```

Design notes:
- FS "injection" = pass a directory; tests write fixtures into `tempfile::tempdir()`. No FS trait.
- Use ratatui's re-exported crossterm (feature default in 0.30) to avoid version skew; `ratatui::run` handles raw mode and restore-on-panic.
- Pane title shown dim on the row when it starts with `✳ ` (prefix stripped), else nothing.

### Ordered TDD behaviors (one failing test → minimal impl → refactor, each)

Outside-in, `tests/picker.rs` first:
1. No agents: renders "No Claude Code sessions in this tmux server"; `q` returns.
2. Two agents: one row each showing `label`; first row highlighted with `> `.
3. `j` / `Down` moves highlight down.
4. `k` / `Up` moves up; both clamp at the ends.
5. `Enter`: `FakeTmux` recorded `focus(%53)` and `run` returned.
6. `Enter` on empty list: no focus call, does not exit.
7. `Esc` and `Ctrl-c` return without focusing.
8. Row shows `label  <title minus ✳>`; no title → label only.

`tests/discover.rs`:
9. Interactive record with a live pid and a matching pane → one `Agent` with pane, session, window_index, title, cwd.
10. Drops `kind != Interactive` (incl. Unknown).
11. Drops records without `tmux`.
12. Drops records whose pane is not in `panes`.
13. Drops dead pids.
14. `label` is basename of cwd; two agents sharing a cwd get the `·session:window` suffix.
15. Sorted by `(session, window_index)`.

`tests/registry.rs`:
16. `load` parses the verbatim real sample (extra fields ignored).
17. Skips `.key` files and malformed JSON.
18. Missing/unknown `status`/`kind` → Unknown.
19. `sessions_dir` honors `CLAUDE_CONFIG_DIR`, defaults to `~/.claude/sessions`.

`src/tmux.rs` unit tests:
20. `parse_pane_ref` happy path; garbage and empty → None.
21. `parse_list_panes` on a two-line sample with a space-containing title.
22. `focus_args` == `["switch-client","-Z","-t","%53"]`; `list_panes_args` includes `-L <sock>` only when set, and `-a -F` with the six fields.

`src/cli.rs`, `src/process.rs`:
23. `parse([])` → Tui; `parse(["bogus"])` → Err mentioning usage.
24. `is_alive(own pid)` true; reaped child false.

Live, `#[ignore]`:
25. `tmux -L tmux-agents-test new-session -d`; `CliTmux::list_panes()` returns one pane with `%` id; `kill-server` in a drop guard. Focus can't be live-tested (no attached client in CI), hence test 22.

Then `main.rs` wiring, install, tmux binding, docs.

### Dependencies

Versions verified 2026-09-07 against crates.io and static.rust-lang.org: Rust stable is 1.98.1 (installed; 1.99 still beta), ratatui 0.30.2, nix 0.31.3, tempfile 3.27.

```toml
[package] name = "tmux-agents"  edition = "2024"  rust-version = "1.98"
[dependencies]
ratatui = "0.30"                 # 0.30.2, re-exports crossterm 0.29
serde = { version = "1", features = ["derive"] }
serde_json = "1"
nix = { version = "0.31", features = ["signal"] }
[dev-dependencies]
tempfile = "3"
```
No clap, no dirs, no anyhow (not needed through Phase 3). Run `cargo update` at the start of each phase and bump the CI toolchain pin together with `.tool-versions` when a new Rust stable lands.

### Quality gates (save as project memory when implementation starts)

```
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

### CI: `.github/workflows/tmux-agents.yml`

- Triggers: push to `main` and PRs, path-filtered to `tmux-agents/**` and the workflow file.
- `concurrency: cancel-in-progress` keyed on ref (auto-sync pushes on every save).
- ubuntu-latest, `dtolnay/rust-toolchain` pinned to `1.98.1` with `rustfmt, clippy`, `Swatinem/rust-cache`.
- Steps: fmt check → clippy `-D warnings` → `cargo test` → `apt-get install tmux` → `cargo test --test tmux_live -- --ignored`.

### Repo integration, in order

1. Append `tmux-agents/target/` to `.gitignore` **before** `cargo new` (auto-sync would otherwise commit build output).
2. Create the crate, run the TDD cycles above.
3. Install: `cargo install --path ~/dotfiles/tmux-agents --root ~/.local --locked` → `~/.local/bin/tmux-agents` (on the tmux server's PATH). No symlink into `scripts/` (would dangle on fresh machines since `target/` is ignored).
4. `.tmux.conf`, after line 68:
   ```
   # Claude Code session picker: <CR> focuses the selected pane, q/Esc closes.
   bind -n M-c display-popup -E -w 50% -h 40% "tmux-agents"
   ```
5. `SETUP_MACOS.md`: add an install subsection after "Customize tmux sessionizer".
6. Add the CI workflow.

### Verification

- `cargo test` green locally; CI green on the auto-synced push.
- `tmux source-file ~/.tmux.conf`, press `M-c` from a pane in another session: popup lists every live Claude Code session (compare with `ls ~/.claude/sessions/*.json`), `Enter` lands on the right pane across sessions, `q` closes.
- Kill a session's Claude Code process, reopen: row gone. Run `tmux-agents` outside tmux: clear error on stderr, exit 1.

### Risks

- Registry format is internal; a field rename degrades to "no sessions". Guard = verbatim fixture test; re-copy it on Claude Code version bumps.
- Pane id collision across tmux servers (`-L`): mitigated by pid liveness; match on `(session, pane)` if it bites.
- Every save triggers a commit, push and CI run during TDD; path filter + cancel-in-progress bound the cost.
- Edition 2024 with clippy `-D warnings` can break on toolchain bumps; CI pin matches asdf.

---

## Phase 2 — agent state (revised after Phase 1 retro, 2026-09-07) — DONE 2026-09-07

Goal: each row shows a colored dot + one word, herdr-style, refreshed live.

### Phase 1 learnings applied
- Manual detached-window check caught a bug 25 tests missed (blank until first key). Phase 2 starts with an automated end-to-end test.
- Popup is a static snapshot; live state needs a tick-driven loop anyway.
- Only `busy`/`idle` were observed in the registry; `waiting`/`shell` semantics must be verified before "blocked" is built on them.
- Two sessions in one window share the `·session:window` suffix.

### Row shape (decided)
Single line: `● dotfiles  working  <title>` — dot colored by state, word dim, title dim. Two-line rows would halve a 40% popup to ~7 rows.

### State mapping

| registry `status` | word | dot | ANSI color |
|---|---|---|---|
| `busy`, `shell` | working | ● | yellow |
| `waiting` | blocked | ◉ | red |
| `idle` | idle | ○ | dim, theme fg (green clashed with purple themes) |
| unknown | ? | ○ | dark gray |

ANSI 16-color palette only, so ghostty-mirror themes carry through. No "done" state: interactive sessions vanish from the registry on exit; revisit via a `Stop` hook if idle proves insufficient.

### Sort
By state group (blocked, working, idle, unknown), then session, then window index. Selection is kept by pid across refreshes and filter edits.

### Architecture changes
- `app::run` consumes `Input { Key(KeyEvent), Tick }`; main maps `event::poll(500ms)` timeouts to `Tick`.
- `App::refresh(Vec<Agent>)` replaces agents, re-sorts, preserves selection by pid (falls back to first).
- `main` builds a `Source` closure (`load` + `list_panes` + `discover`) called on every `Tick`.
- Test helper gains `cell(x, y) -> &Cell` for glyph + style assertions.

### TDD behaviors (in order)
1. E2E (`tests/e2e.rs`, `#[ignore]`): tmux `-L` server, fixture session JSON pointing at its pane with our own pid, run the installed-from-target binary in a window with `HOME`=tempdir, `capture-pane` shows the row. Also asserts the screen is drawn before any key.
2. Suffix collision: same window → append pane id (`·session:window.%pane`).
3. Row shows `●`/`○` glyph and state word for each status.
4. Dot color per state; word and title dim (style assertions via `cell`).
5. Sort by state group, then session/window.
6. `Tick` re-reads the source and replaces rows.
7. Selection preserved by pid across a refresh that reorders.
8. Selection falls back to first when the selected agent disappears.
9. Filter re-applies after a refresh.
10. main wiring: `event::poll` + `Tick`; reinstall; live check.

### Outcome
- `waiting` verified from the binary: set when a permission dialog, elicitation/AskUserQuestion, sandbox or worker request, or a local command dialog is open. No writer for `shell` found; mapped to working.
- E2E test proven by mutation: reverting the draw-before-read fix makes it fail.
- All 10 behaviors shipped; 42 tests + 2 ignored.

### Verification (as planned)
`waiting` semantics checked empirically before task 3 (permission prompt in another session → `cat ~/.claude/sessions/<pid>.json`). If it does not flip, blocked moves to a `PermissionRequest` hook writing `~/.claude/sessions/<pid>.blocked` and Phase 2 scope is re-discussed.

---

## Phase 3 — tmux status line (revised after Phase 2 retro, 2026-09-07) — DONE 2026-09-07

Goal: `status-right` shows per-state counts, refreshed every second.

### Decisions
- Glyphs, not emoji: `●2 ●1 ○3` with tmux markup (`#[fg=yellow]`), same glyphs as the popup, themed by ghostty-mirror. Emoji ignore the theme and misalign by a cell across terminals.
- Order: blocked (bold), working, idle. Unknown counted as idle-glyph grey only when present. Zero counts hidden; empty output when no agents so the separator vanishes.
- Output is tmux format markup (parsed by `#()`), never ANSI escapes.
- `status-interval 1` only after measuring the other `#()` widgets; any widget over ~50 ms gets an internal TTL cache so it stays effectively at 5 s.
- Both `status-right` definitions get the segment: `.tmux.conf` and the untracked `~/.tmux_work.conf` on this machine.

### Architecture
- `cli::Command::Status` → `status::render(&[Agent]) -> String` (pure); main prints and exits.
- `CliTmux` honors the `TMUX` env var socket implicitly (tmux CLI does), so the e2e harness can drive the subcommand against the scratch server.
- Reuses `load` + `list_panes` + `discover` unchanged.

### TDD behaviors
1. `cli::parse(["status"])` → `Command::Status`.
2. `render` of one working agent → `#[fg=yellow]●1#[default]`.
3. Order blocked, working, idle; blocked bold.
4. Zero counts hidden; no agents → empty string.
5. Unknown status shown as grey `○n`.
6. E2E: `tmux-agents status` in the scratch server with a fixture prints the expected markup.
7. Wiring: main dispatch; `.tmux.conf` + `~/.tmux_work.conf` segment; `status-interval 1`; widget caches as measured.

### Outcome
- Widgets measured before: git-widget 180 ms, kube_status 90 ms, tailscale_status 110 ms. All wrapped in `scripts/tmux-cached 5`, so at `status-interval 1` they still run every 5 s. `tmux-agents status` is 10–30 ms warm.
- e2e harness bug found: both e2e tests shared one socket name and killed each other's server when run in parallel. Sockets are now per test.
- `tmux display -p` cannot evaluate `#()`; verification is by cache-file mtimes advancing and by eye.
- 6 render/cli tests + 1 e2e; 51 tests total, 3 ignored live.
- Zero-session handling (later the same day): `status` prints `none` after the sparkle so the config-level `|` never dangles; the empty popup shows a hint and `n` splits a claude pane into the caller's window (`command-prompt` from a popup shows the prompt but never runs its command).

### Verification (as planned)
- `time` each widget in status-right before/after.
- Live: status bar shows counts matching the popup; goes blank with no sessions.

---

## Post-phase review (2026-09-07)

Twelve items from the review, each under /tdd:
1. Title strip generalised to any leading glyph (spinner-safe).
2. Stable row order across refreshes; append new, drop gone.
3. `Mode { Normal, Filter, Help }` replaces two booleans.
4. Release profile: lto, codegen-units=1, strip (981 → 719 KB).
5. `insta` snapshot tests for the four layouts.
6. `1`–`9` focus a row directly.
7. Age since `statusUpdatedAt` on the right of each row.
8. Blocked reason from `waitingFor`, red, before the title.
9. Collision suffixes removed; a `session:window` column was tried and dropped the same day (noise).
10. `~/cwd` fallback when untitled; ellipsis truncation so the right column always fits (fixed a real overflow found by the snapshot).
11. Popup 70% × 60%.
12. Zero-session e2e for `status`.

## Backlog (proposed 2026-09-07, not started)

Ranked by value. Suggested order: 5, 8, 7, 4, 3, 2, 1, then decide on 6 and 9.

### High value
1. **Preview pane.** Split the popup into list | `capture-pane` of the selected session, refreshed on the tick. Lets you read a blocked session's permission prompt or question without switching. Medium: one new `Tmux` trait method (`capture(pane) -> Vec<String>`), a horizontal layout, a fake in tests, snapshot for the layout. Consider `p` to toggle it and remembering the choice in the session. — DONE 2026-09-07 (`p` toggles; shown only at ≥100 columns; capture failures degrade to no preview; choice not persisted).
2. **Kill from the popup.** `x` on a row prompts `kill <label>? y/n`, then `kill-pane`. Use case: long-idle sessions. Small, but first destructive action: new `Mode::Confirm(PaneId)`, tests that `n`/Esc do nothing, `y` calls the fake once. — DONE 2026-09-07 (popup stays open and refreshes after the kill).
3. **Blocked names in the status line.** When blocked > 0, print `◉ webapp` (cap at two names, then `+n`) instead of `◉ 1`. Small; pure change in `status::render`. — DONE 2026-09-07.

### Robustness
4. **Move `tmux-cached` into the binary** as `tmux-agents cached <ttl> -- <cmd>`. The shell version is untested, spawns bash + `shasum` + `stat` per widget per tick, and its `stat -f`/`stat -c` branch is a portability risk. Rust version: one process, cache under `~/.cache/tmux-agents/`, tests with a tempdir and a fake clock. Then update both `status-right` definitions and delete `scripts/tmux-cached`. — DONE 2026-09-07 (`src/cached.rs`, `tests/cached.rs`; dotfiles `.tmux.conf`, `~/.tmux_work.conf`, docs updated, script deleted).
5. **CI builds release.** Add `cargo build --release` to `.github/workflows/tmux-agents.yml` so the lto/strip profile is compiled in CI, not only at `cargo install`. Trivial. — DONE (already in `ci.yml` at import; verified 2026-09-07).
6. **Stale-busy detection.** A hung Claude process leaves `busy` forever. Dim a row `busy` for more than ~30 min, or show `busy?`. Small; needs a threshold constant and a test with a fixed `now`. — DONE 2026-09-07 (`stale?` word + dim glyph via `agents::STALE_AFTER`; status line unchanged).

### Polish
7. **Oldest blocked first.** Within the blocked group sort by age descending so the most neglected prompt is row 1 and `M-c 1` goes there. Trivial change to `sort_key`; note the stable-order merge means it only affects the initial order. — DONE 2026-09-07 (undated blocked rows sort after dated ones).
8. **Version 0.2.0.** Help footer shows `v0.1.0` while the feature set has doubled. Trivial. — DONE 2026-09-07.

### Bigger, discuss first
9. **Proactive alert.** `tmux-agents watch`: long-lived process that runs `tmux display-message "◉ dotfiles needs input"` on the idle/busy → blocked transition. Needs a launcher (launchd, or `run-shell -b` at tmux start), debouncing, and a decision on whether the status line already covers it. Only worth it if prompts are being missed today. — DONE 2026-09-07. Decisions: launcher is `run-shell -b "tmux-agents watch"` in `.tmux.conf` (no launchd); one watcher per server via a pid lock; the watcher exits with the server; debounce is a 3s per-pid quiet window plus a silent first poll; clients already on the blocked pane are skipped. The status line (item 3) shows *who* is blocked, the watcher adds *when* it happened.

### Considered and dropped
- `session:window` column: added and removed the same day, read as noise.
- Model / cost / context% per row: not in the registry; would need the statusline hook to write a side file.
- Mouse support: keyboard-first workflow.
- Fuzzy filter: substring on label + title has been enough.

## Retrospective checkpoints

After each phase, before planning the next:
1. Did the registry format hold up? Any missing sessions, stale rows, wrong panes?
2. TDD friction: which behaviors were awkward to test; adjust seams.
3. CI signal: flaky tests, runtime.
4. UX: popup size, keys, naming collisions.

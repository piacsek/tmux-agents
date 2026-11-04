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

## This repository is public

No employer data, ever: no org or repo names, PR numbers or titles, logins, hostnames,
ticket prefixes. Screenshots and TUI tests use invented session names; plans and docs use
`<owner>/<repo>` placeholders. `scripts/scrub-check.sh` (first gate) fails when a tracked
file matches the local denylist at `~/.config/tmux-agents-dev/denylist` (kept outside the
repo on purpose; the check is skipped where the file is absent, so CI does not enforce it).

## Layout

```
src/main.rs      CLI dispatch, event loop wiring (event::poll → Input::Tick every 500 ms)
src/cli.rs       `tmux-agents` (TUI) | `status` | `watch` | `config` | `cached <ttl> -- <cmd>` | `--help` | `--version`
src/config.rs    Config (serde + toml), defaults, XDG path resolution, `label_map`
scripts/gates.sh              the quality gates; fails loudly, never pipe it through tail
scripts/scrub-check.sh        denylist grep over tracked files (see "This repository is public")
scripts/dev-install.sh        release build symlinked as ~/.local/bin/tmux-agents-dev
scripts/ship.sh               gates + dev-install + commit + push, aborts on any failure
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
src/text.rs      truncate() shared by rows and status names
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
  an unreadable file or unknown key is an error naming the file (`deny_unknown_fields`
  on every table), always printed to stderr. Only `config` exits 1 on it: `status`
  prints `#[fg=red,bold]⚠ config#[default]` so the segment never goes blank, the
  TUI runs on defaults with the error in the footer until the first key, `watch`
  runs on defaults, and `cached` ignores the config entirely. `Config::validate` rejects `tick_ms = 0`, `interval_ms = 0`
  (both would busy-loop spawning tmux) and a `split_percent` outside 1..=100,
  naming the key. `tmux-agents config` prints the effective TOML and doubles as the
  reference. Constants that used to live in code (tick, preview
  width/split, status prefix, watch timings, new-pane command, labels) now come from
  `App::config`, `CliTmux::with_new_pane`, `status::render(_, &StatusLine)`,
  `watch::run(_, _, _, &Watch)` and `discover(_, _, _, _, &labels)`. Preview is **off**
  by default; tests that need it pass a `Config` with `preview.enabled = true` via
  `Picker::with_config`, and the e2e preview test sets `TMUX_AGENTS_CONFIG`.
- **tmux errors stay inside the popup.** A failed focus, new pane or kill sets
  `App::error`, drawn red in the footer in place of the mode text; the next key
  clears it and the popup stays open (a `display-popup -E` closes on exit, so an
  error that exited was invisible). Only `source()` failures (tmux server gone)
  still end `run()`.
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
- **No staleness marker.** Claude Code writes `statusUpdatedAt` (and
  `updatedAt`, which mirrors it) only on status transitions, never as a
  heartbeat, so "working for 40 min" is indistinguishable from "hung for 40
  min". The old `stale?` word flagged every long autonomous run; the age column
  on the right is the only hint now. `[picker] stale_after_minutes` was removed
  with it and is rejected as an unknown key.
- **Title stripping** removes any leading non-alphanumeric glyph plus space
  (Claude uses `✳` and spinner glyphs); a plain hostname title becomes `None`
  and the row falls back to `~/cwd`.
- **Status-line output is tmux markup**, never ANSI. Zero sessions prints
  `none` so a config-level separator never dangles. The blocked segment names
  the sessions (`◉ webapp dotfiles +1`, two names then `+n`, oldest prompt
  first) while working and idle stay counts. Names longer than
  `status.max_label` (16) are cut with `…` via `text::truncate`, the same
  helper the picker rows use, so a long directory cannot overflow
  `status-right-length`.
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
  the next tick instead of being served for a whole TTL. Every successful
  store also prunes sibling entries (16-hex names only, never `watch-*.pid`)
  whose mtime is older than a day, so per-path widgets do not pile up files. `cached::private_dir`
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

## Working agreements

All guidelines live in this file, versioned with the code; nothing binding lives only in an
agent's private memory. When the user says "update the guidelines", edit this file.

- **"Work autonomously" means the whole road to a release, without pausing**, not even for
  the retro: its outcome is written into `PLAN.md` in step 4 and the user reads it there. When
  the user says it (or "autonomous", "march towards release"), assume this flow and ask only
  when a decision is genuinely theirs:
  1. Plan in `plans/phase-N-<topic>.md` (next free N in `PLAN.md`), in a fresh worktree. The
     user's request stands in for plan approval: file the plan as an issue right away (see
     "Approved plans become GitHub issues") and put `Closes #N` in the PR body.
  2. `/my-tdd autonomous`: one task per red-green-refactor cycle, each shipped with
     `scripts/ship.sh` (gates, dev install, commit, push, draft PR on the first push).
  3. `/pr-review` the PR (see "Review before ready"). Fix every must and should finding, TDD
     like any change, ship, and review again. Minor findings are fixed when cheap, else listed
     in the PR body. The loop ends only on a round with no must/should findings, and that
     round is **unprimed**: the reviewers get the PR, the repo and this file, never the list
     of earlier findings or what was decided about them. Never post the review on the PR.
  4. The phase outcome is in `PLAN.md` and, for a behaviour change, the version bump is on the
     branch. Re-read the PR title and body, fix what went stale, `gh pr ready`, wait for CI
     green.
  5. Rebase-merge. For a behaviour change, tag and release per "Releasing", watch `release`
     to green and check the tap formula names the new version.
  6. Remove the worktree, `git pull --ff-only` on `main` in the main checkout and re-run
     `scripts/dev-install.sh` there, so `tmux-agents-dev` is the merged code; then report
     what shipped, with the release link.
  Stop and ask instead of proceeding when: a gate fails for a reason the change did not
  cause, CI fails twice on the same step, a review finding needs a product decision, or the
  next step would force-push, push to `main` or delete a release.
- **Cadence.** When the user has not asked for the full flow above, work still ships per task
  under `/my-tdd autonomous`: one branch per phase (`phase-N-<topic>`), one commit per task
  through `scripts/ship.sh`, no pause between tasks, but it pauses for the end-of-phase retro
  instead of marching on to the release. Retro outcomes go to
  `PLAN.md` (facts, decisions) and here (rules).
- **One worktree per branch, by default.** Start every phase or fix in its own worktree off
  `origin/main`: `git worktree add -b <branch> .claude/worktrees/<branch> origin/main`, then
  `git branch --unset-upstream <branch>` so the first `ship.sh` push creates `origin/<branch>`
  instead of tracking `main` (name the branch: run from the main checkout, the bare form
  unsets `main`'s upstream). Work, gates and ships run from inside it; the main checkout stays on its
  own branch, so two phases (and two agent sessions) never share an index or a `target/`.
  `.claude/worktrees/` is gitignored. Each worktree builds its own `target/` from cold the
  first time. `tmux-agents-dev` points at the worktree that last ran `dev-install.sh`, so
  re-run it where you are testing, and from the main checkout once the worktree is removed.
- **TDD is never skipped.** Every change to `src/` starts with a failing test, run and seen
  red, before the production code is touched; then the smallest change that goes green, then
  refactor. This holds for one-line fixes, security patches, patch releases and "obvious"
  changes alike. Writing the test and the fix in the same edit is not TDD, even when the test
  is then backed out to prove it fails. Show the red run in the update.
- **Draft PR first.** The first ship on a branch opens the draft PR assigned to the user;
  every later commit lands on that PR. Before merging, re-read the PR title and body and fix
  what went stale without rewording the user's edits.
- **Screenshots in PRs.** A PR with a visible change embeds the `docs/*.png` screenshots in
  its body as commit-pinned `raw.githubusercontent.com` URLs.
- **Review before ready.** Marking a PR ready for review means running the
  [`/pr-review`](https://github.com/piacsek/dotfiles/blob/main/.claude/skills/pr-review/SKILL.md)
  skill on the PR first, and acting on what it finds. It runs a Claude and a Codex reviewer
  over the diff and merges their findings; run it from the main session, because the skill
  launches Codex there. Give both reviewers the intent of the change, point them at this
  file, have the Claude reviewer run `scripts/gates.sh` itself rather than trust the claim
  that the gates pass, and forbid edits, commits, and any write to GitHub. When the skill
  asks whether to post the review on the PR, answer no. `gh pr ready` comes after the
  findings are addressed, not before.
- **Merging and releasing.** Rebase-merge the phase PR into `main` when CI is green, then tag
  from `main` (see Releasing). Never push to `main` directly, and never force-push.
- **Catching up with `main` on a pushed branch is a merge, not a rebase.** A rebase rewrites
  the pushed commits and the next `scripts/ship.sh` push is refused; the no-force-push rule
  then leaves only `git reset --hard origin/<branch>`, `git merge origin/main` and a
  cherry-pick of the new work. The rebase-merge into `main` drops the merge commit anyway.
- **Two binaries.** `tmux-agents` on PATH is the Homebrew release, and it is what
  `.tmux.conf` runs; `tmux-agents-dev` is the working tree (`scripts/dev-install.sh`).
  Never `cargo install` the crate into a PATH directory.
- **Plan before code, plans in the repo.** Each phase starts from a written plan, re-planned
  after the previous retro, with its "Verified facts" checked against live tools before use.
  Write each plan to its own file under `plans/` (for example `plans/phase-6-conveyor-guidelines.md`)
  and commit it with the first ship of the phase; never leave a plan only in an agent's private
  plan directory or memory. `PLAN.md` stays the index of phases, retros and the backlog.
- **Approved plans become GitHub issues.** Once the user approves a plan, file it with
  `gh issue create --assignee @me`, title per the ticket rules (user-visible effect, plain
  language), body = the plan verbatim. The phase PR closes it (`Closes #N`).

## Development

Strict TDD: one failing test, minimal code, refactor. Outside-in: start in
`tests/picker.rs` (drives `run()` with scripted `Input`s), drop to unit tests
only for pure helpers. No code comments; names and tests carry the intent.

Gates, all required before a task is done, wrapped by `scripts/gates.sh`:

```
scripts/scrub-check.sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo test -- --ignored        # needs a tmux binary; spawns `tmux -L` servers
```

Ship with `scripts/ship.sh "<message>"`: `cargo fmt`, gates, `dev-install.sh`, commit, push,
all under `set -e`, refusing to run on `main`, opening the draft PR on the first push of a
branch. Do not hand-roll the chain: the interactive shell here is zsh, where `PIPESTATUS` is
undefined and `test "" -eq 0` is true, so a `gates.sh | grep` guard silently passed a clippy
failure into a commit in conveyor on 2026-09-10. Piping `gates.sh | tail` once hid a
`cargo fmt --check` failure here and an unformatted commit slipped through.

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

**A scratch `HOME` does not isolate a launch.** `XDG_CONFIG_HOME` and `TMUX_AGENTS_CONFIG`
outrank `HOME` for the config, `CLAUDE_CONFIG_DIR` for the registry and `XDG_CACHE_HOME` for
the cache, and the user's shell exports `XDG_CONFIG_HOME`. `tests/e2e.rs` strips all four from
the scratch tmux server (its panes inherit the server's environment, not `respawn-pane -e`
alone) and from every direct launch, through `isolated()`; `scripts/screenshots.sh` unsets
them up front. Every scratch server also starts with `-f /dev/null`: the real `~/.tmux.conf`
runs `tmux-agents watch` and can `set-environment` the stripped variables back. Any new
launcher must do the same. Prove it with a hostile run: point all four at a scratch dir
holding a config that changes `[status] prefix` and a fake session, then run
`cargo test --no-fail-fast -- --include-ignored`; repeat with a config that fails to load
(an unknown key), which catches launchers whose assertions survive a valid one; for the tmux config, run the built test
binaries with `HOME` at a scratch dir whose `.tmux.conf` sets those variables and opens a
second session (cargo's asdf shim needs the real `HOME`).

Install: `scripts/dev-install.sh` builds the release binary and symlinks it as
`~/.local/bin/tmux-agents-dev`; `tmux-agents` on PATH is always the Homebrew release. A
milestone is done only after the gates pass **and** `dev-install.sh` has run, so
`tmux-agents-dev` is the committed code. Try a change in tmux with the dev binary by name
(`tmux-agents-dev`, `tmux run-shell "tmux-agents-dev status"`); the popup and status line
keep running the release. `tmux display -p` cannot evaluate `#()`; verify status-line output
with `tmux run-shell` instead.

## Releasing

Releases are GitHub Releases built by `.github/workflows/release.yml` on a `v*` tag:
one flat tarball per target (`tmux-agents-<target>.tar.gz` holding just the
binary; targets `aarch64-apple-darwin`, `x86_64-apple-darwin`,
`x86_64-unknown-linux-gnu`) plus `.sha256`, with generated notes. Asset names
carry no version so the README's `releases/latest/download/` one-liner stays
valid; do not rename them. Both macOS targets build on `macos-latest` (the Intel
one cross-compiled); the `macos-13` runner label is retired and a job asking for
it queues forever. The workflow
refuses a tag whose version differs from `Cargo.toml`. Every action in both
workflows is pinned to a commit SHA with the tag in a trailing comment; bump
pins by resolving the tag with `gh api repos/<owner>/<repo>/git/ref/tags/<tag>`
(dereference annotated tags via `git/tags/<sha>`). Workflows default to
`contents: read`; only `publish` gets `contents: write` plus `id-token` and
`attestations: write` for `actions/attest-build-provenance`, which is what makes
`gh attestation verify <tarball> --repo piacsek/tmux-agents` work. CI's `check` job runs
`cargo test -- --include-ignored` once (unit, TUI and tmux e2e; the e2e tests take the debug
binary from `CARGO_BIN_EXE_tmux-agents`, so no release build there). The `audit` job runs a
prebuilt, checksum-verified `cargo-audit` from `taiki-e/install-action` with `fallback: none`:
never `cargo install` a tool in CI (`rustsec/audit-check` compiled it from source on every
run, about 3 minutes). A red `audit` check is the only advisory signal; the action used to
open an issue instead.

Only behaviour changes get packaged. A release (version bump, tag, tarballs, tap) follows a
change a user can observe in the binary: a key, a row, a default, a fix, a config key. Docs,
CI, tests, scripts, refactors and guideline edits merge to `main` without a bump and ride the
next behaviour release. Do not tag a docs-only or CI-only `main`.

1. Bump `version` in `Cargo.toml` (`Cargo.lock` follows on the next build) and
   refresh the help snapshot: `INSTA_UPDATE=always cargo test --test snapshots`.
2. Ship `Release vX.Y.Z` on the phase branch, rebase-merge the PR into `main` when CI is green.
3. `git tag vX.Y.Z && git push origin vX.Y.Z`, then `gh run watch` until the
   `release` workflow is green and `gh release view vX.Y.Z` lists three tarballs.
4. The `tap` job then regenerates `Formula/tmux-agents.rb` in
   `github.com/piacsek/homebrew-tap` (local clone: `~/projects/homebrew-tap`) with
   `scripts/homebrew-formula.sh <version>` and pushes with the `TAP_TOKEN` repo
   secret (a fine-grained PAT with contents:write on homebrew-tap). If that job
   fails, run the script by hand and push the tap. `brew install
   piacsek/tap/tmux-agents` must keep working; check with `brew audit --strict
   --online --formula piacsek/tap/tmux-agents` after changing the script.

Installed copies: Homebrew puts the release binary in `$(brew --prefix)/bin/tmux-agents`;
dev builds are only reachable as `tmux-agents-dev`. After a release, `brew upgrade
tmux-agents` is what moves the popup and status line to the new code.

Never force-push a tag that has a release: the push re-runs `release.yml`, republishes the
assets and rewrites the tap formula for that old version. If a released tag must move,
delete the release and the tag first, or bump the version instead.

Semver: minor for new keys/subcommands/config, patch for fixes, major on a
config-format break.

## tmux integration (lives in the user's dotfiles, not here)

```
bind -n M-c display-popup -E -w 70% -h 60% "tmux-agents"
set -g status-right " #(tmux-agents status) #[fg=white]|#[default] #(tmux-agents cached 5 -- ~/dotfiles/scripts/tmux-git-widget '#{pane_current_path}') …"
set-option -g status-interval 1
run-shell -b "tmux-agents watch"
```

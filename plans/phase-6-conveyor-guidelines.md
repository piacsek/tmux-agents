# Phase 6: conveyor's guidelines (2026-11-03)

Trigger: conveyor's working agreements, test traps and CI moved on since tmux-agents adopted its
flow in phase 5. Bring the ones that apply here. No behaviour change, so no release.

## Verified facts

- `scripts/ship.sh`, `gates.sh`, `dev-install.sh` and `scrub-check.sh` already match conveyor
  apart from names.
- The user's shell exports `XDG_CONFIG_HOME`. With a config there that sets `working_style` and
  `prefix`, and `TMUX_AGENTS_CONFIG`, `CLAUDE_CONFIG_DIR`, `XDG_CACHE_HOME` pointing at a scratch
  dir, 5 of the 6 e2e tests fail: they set `HOME` only, and the tmux server they start copies
  the caller's environment into every pane.
- CI's `audit` job uses `rustsec/audit-check`, which compiles cargo-audit from source on every
  run; `check` runs `cargo test` and `cargo test -- --ignored` separately plus an unused
  `cargo build --release`.

## Tasks

1. e2e and `screenshots.sh` never read the caller's config, registry or cache: strip
   `XDG_CONFIG_HOME`, `TMUX_AGENTS_CONFIG`, `CLAUDE_CONFIG_DIR`, `XDG_CACHE_HOME` from the
   scratch tmux server and every direct launch of the binary. Red: the hostile run above.
2. `AGENTS.md`: the "work autonomously" flow, one worktree per branch, TDD is never skipped,
   review before ready, catching up with `main` is a merge, plans in `plans/`, only behaviour
   changes are released; `.claude/worktrees/` gitignored; the config-isolation trap.
3. CI: prebuilt cargo-audit through `taiki-e/install-action`, one
   `cargo test -- --include-ignored`, no release build.

Not taken: conveyor's `CHANGELOG.md` (it feeds conveyor's own changelog pane) and its README
lint (`tests/readme.rs`, tied to conveyor's key table).

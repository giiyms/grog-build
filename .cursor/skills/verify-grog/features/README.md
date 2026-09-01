# Feature map (verify-grog)

This directory is the maintained verification source for grog. A proof that drives one convenient entry point is incomplete when this map lists others — drive what you can, and **skip** the rest with the unmet precondition named.

## Baseline preconditions

Every drive, including CLI-only:

1. Disposable `GROG_HOME=/tmp/grog-verify-$RUN_ID` (skill `scripts/launch.sh`). Never `~/.grok`. Never the user's `~/.grog`.
2. `unset GROK_HOME` so dump `xai-dirs` cannot redirect into official grok.
3. `scripts/doctor.sh` is green (binary is `grog`, home tagged `.verify-grog-owned`).
4. **Never drive an instance you did not start.** No attaching to an existing pager, leader socket, or tmux session. If `scripts/drive-tmux.sh start` finds its session already alive, it refuses.

TUI drives also need `--no-leader` (the helper passes it) so this process does not elect `GROK_HOME/leader.sock` for anyone else.

## Driving conventions

- Harness: tmux PTY via `scripts/drive-tmux.sh`, or a one-shot CLI under the same `GROG_HOME`.
- Prefer slash names and key chords (`/help`, `Ctrl+P`, `Esc`, `Quit`) over screen coordinates.
- Ready (TUI): pane contains `Quit`. Ready (CLI): `grog --version` exits 0 and starts with `grog `.
- Isolate every process with `GROG_HOME` + unset `GROK_HOME`. Pin `GROK_LEADER_SOCKET` under the disposable home if you omit `--no-leader`.

## Proof and skip reporting

**Pass.** Capture action + visible result + `$GROG_HOME` side effects in `.cursor/skills/verify-grog/artifacts/`. Cleanup must leave those files.

**Skip.** Allowed when a named precondition is unmet (no `claude` on PATH, no Codex tokens, no GitHub Darwin artifact on Linux, login gate blocking a session-scoped command). Write `artifacts/skip-<feature>.md` with the precondition. A skip is not a pass.

**Fail.** Unexpected visible state, writes under `~/.grok`, or cleanup deleting artifacts.

## Feature entry contract

Each file starts with an H1 and one paragraph of user-visible behavior, then exactly four H2s in this order:

1. `Sub-features`
2. `How to get to it (user POV)`
3. `Driving it with tmux/PTY` (CLI-only features still use this heading and say when tmux is unused)
4. `Gotchas`

Keep implementation crates out of the map. Name what the user sees and which files on disk under `$GROG_HOME` prove it.

## Features

- [Identity and home](identity-and-home.md) — `grog --version`, `$GROG_HOME`, never `~/.grok`
- [Help](help.md) — `/help` / `Ctrl+P` command palette
- [Doctor](doctor.md) — `grog doctor` and `/doctor`
- [Council](council.md) — `/council` multi-model deliberation (needs provider logins)
- [Advisor](advisor.md) — `/advisor` sidecar (status/toggle local; consults need a seat)

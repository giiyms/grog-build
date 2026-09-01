---
name: verify-grog
description: >-
  Drive grog (Grok Build TUI/CLI fork, public command grog) on its real
  pager TUI and CLI to prove user-visible behavior. Use when verifying
  identity, GROG_HOME isolation, slash commands (/help, /council, /advisor,
  /restart, /update, /doctor), or native providers — never official grok's ~/.grok.
---

# verify-grog

Cold-start skill for the next agent. This repo is **grog**: a multi-provider fork of Grok Build. Public binary name is `grog`. argv0 also accepts `grok` and `agent`; clap still treats those as the same binary. Cargo artifact alias: `xai-grok-pager`. Do not install or overwrite official xAI `grok` (`~/.grok`). Darwin/Mac compile is out of scope; Linux `cargo` is the verification host.

Feature map: [`features/README.md`](features/README.md). Proof artifacts: [`artifacts/`](artifacts/).

## Interview (ground truth)

- **Surface (primary):** fullscreen pager TUI (`xai-grok-pager`). User types at the composer; `/` opens the slash menu; `Ctrl+P` is the command palette.
- **Other surfaces:** CLI subcommands (`grog doctor`, `grog login [codex|claude|agy]`, `grog models`, `grog update`, `grog --version`); headless `grog -p "…"`; ACP/editor embed. Native inference crates: `grog-providers`, `grog-claude-bridge`, `grog-antigravity`, `grog-codex`. Council is `/council` (shell builtin → `council.rhai`).
- **Run:** `cargo build -p xai-grok-pager-bin --bin grog` then `target/debug/grog`. Documented equivalent: `cargo run -p xai-grok-pager-bin` (`default-run = "grog"`). Local builds need **protoc**. `bin/protoc` is a DotSlash wrapper (`cargo install dotslash`); if that is missing, set `PROTOC` to a real `protoc` 29.x (same pin as `bin/protoc`).
- **Drive:** in-repo harness `xai-grok-pager-pty-harness` (Rust tests, mock inference, isolated `$HOME` + `GROK_HOME`). Agent verification uses **tmux + PTY** via `scripts/drive-tmux.sh` against the real `grog` binary. Stable handles, not coordinates: prompt slash names, `Quit` on welcome, `New Session` inside the command palette, `Ctrl+P` (`0x10`), `Esc`, `Enter`.
- **Observe:** tmux pane captures, CLI stdout/exit codes, files under `$GROG_HOME` (`sessions/`, `config.toml`, `auth/`, `leader.sock`). Welcome ready-signal used by the PTY e2e suite: substring **`Quit`** (case-sensitive; the authenticating hint uses lowercase `quit`).
- **Isolate:** `$GROG_HOME` then `$GROK_HOME`, else `~/.grog`. Official grok stays in `~/.grok` and must never be created or written. Two TUIs may share a leader socket under one home — **do not double-drive a home you did not start**. Each verify run uses `/tmp/grog-verify-$RUN_ID`, unsets `GROK_HOME`, and passes `--no-leader` (or pins `GROK_LEADER_SOCKET` under that home).

## Launch

Build once. There is no long-lived server. Each drive is a new process in its own tmux session / PTY.

```sh
export VERIFY_RUN_ID="${VERIFY_RUN_ID:-$RANDOM}"
# optional: GROG_BIN=/abs/path/to/grog
.cursor/skills/verify-grog/scripts/launch.sh
```

That script:

1. Forces `GROG_HOME=/tmp/grog-verify-$VERIFY_RUN_ID` (refuses `~/.grok` / `~/.grog`).
2. Unsets `GROK_HOME`.
3. Ensures `PROTOC` (DotSlash `bin/protoc`, PATH, or downloads protoc 29.3).
4. Runs `cargo build -p xai-grok-pager-bin --bin grog` from the repo root.
5. Writes `.verify-grog-owned` into the disposable home.

**Ready (CLI):** `test -x target/debug/grog` and `./target/debug/grog --version` prints a line starting with `grog ` (see `xai_grok_version::PRODUCT_CLI_NAME`).

**Ready (TUI):** after `scripts/drive-tmux.sh start`, wait until the pane contains **`Quit`** (authenticated welcome menu; case-sensitive) **or** **`Waiting for approval`** / **`ctrl+q  quit`** (login gate when no xAI/native session exists). Budget ~20s cold. `--no-leader` is required so this process does not elect a shared leader. `drive-tmux.sh` targets `${session}:0.0` (not `=session` — that is a session match and `capture-pane` fails with `can't find pane`).

**Without vendor credentials** this Linux VM hits the login gate (`Waiting for approval...`), not the welcome `Quit` menu. CLI identity (`grog --version`, `grog doctor`) still works. Do not complete browser login during verification. `/council` live deliberation does **not**.

**TUI argv used by this skill:**

```sh
GROG_HOME=/tmp/grog-verify-$VERIFY_RUN_ID \
GROK_LEADER_SOCKET=/tmp/grog-verify-$VERIFY_RUN_ID/leader.sock \
unset GROK_HOME
target/debug/grog --no-leader
```

Optional: `--minimal` (scrollback-native; idle sentinel `minimal · /help`). Headless: `grog -p "…"` (needs a reachable model; skip without creds).

**Teardown:** `scripts/cleanup.sh` (see Cleanup). Do not leave the tmux session running across drives unless the feature map says so.

## Doctor

Run first whenever anything looks off. Read-only: process we started, identity, home owned by us.

```sh
.cursor/skills/verify-grog/scripts/doctor.sh
```

Pass means:

- `target/debug/grog --version` begins with `grog `.
- `$GROG_HOME` is `/tmp/grog-verify-*` and contains `.verify-grog-owned`.
- Official `~/.grok` is either absent or was not selected as home.

Product command (also maps to feature `doctor`):

```sh
GROG_HOME=/tmp/grog-verify-$VERIFY_RUN_ID unset GROK_HOME
target/debug/grog doctor
```

Stdout starts with `Grok Doctor` (dump terminal diagnostics) then a `Grog providers` block from `grog-providers::doctor` (`claude-bridge`, `antigravity`, `codex`, plus Privacy lines). Missing `claude` / `agy` / Codex tokens is **not** a launch failure.

## Drive

Prefer `scripts/drive-tmux.sh`. Do not drive a tmux session or `$GROG_HOME` you did not create. Do not `pkill grog`.

```sh
export VERIFY_RUN_ID=…   # same id as launch
.cursor/skills/verify-grog/scripts/doctor.sh
.cursor/skills/verify-grog/scripts/drive-tmux.sh start
.cursor/skills/verify-grog/scripts/drive-tmux.sh wait 'Waiting for approval' 25 \
  || .cursor/skills/verify-grog/scripts/drive-tmux.sh wait 'Quit' 5
.cursor/skills/verify-grog/scripts/drive-tmux.sh type '/help'
.cursor/skills/verify-grog/scripts/drive-tmux.sh enter
.cursor/skills/verify-grog/scripts/drive-tmux.sh wait 'New Session' 10
.cursor/skills/verify-grog/scripts/drive-tmux.sh capture help-palette.txt
```

Raw handles from this tree:

| Action | How |
| --- | --- |
| Slash command | type `/help` then Enter (same for `/council`, `/advisor status`, `/restart`, `/update`, `/doctor`, `/quit`) |
| Command palette | `Ctrl+P` (`scripts/drive-tmux.sh keys $'\x10'`) — same overlay as `/help` |
| Dismiss overlay | `Esc` (`keys $'\x1b'`). Palette input mode may need two Esc. |
| Welcome quit | menu label `Quit`; or `/quit` / `/exit` |
| Model picker | `/model` or `Ctrl+M`; advisor slot: `/advisor model` |
| Settings | `F2` or `/settings` |

In-repo Rust equivalent (do not require it for agent proofs): `xai-grok-pager-pty-harness` `PtyHarness::inject_keys`, `wait_for_text("Quit")`, `wait_for_text("New Session")`. Tests seed `GROK_HOME` under a fake `$HOME` and a mock inference server; **this skill uses `GROG_HOME` and never that sandbox layout for live proofs**.

CLI drives (no tmux):

```sh
GROG_HOME=/tmp/grog-verify-$VERIFY_RUN_ID
unset GROK_HOME
target/debug/grog --version
target/debug/grog doctor
target/debug/grog models    # lists catalog ids including claude-bridge/ antigravity/ codex/
```

`grog login codex` copies `~/.codex/auth.json` into `$GROG_HOME/auth/codex.json`. Do not run login during verification unless the feature file says so — it can write the user's Codex tokens into the disposable home (acceptable) but must still never touch `~/.grok`.

## Evidence

Put proof under `.cursor/skills/verify-grog/artifacts/` so it rides the PR. Cleanup must not delete this directory.

Minimum for a pass:

1. **Action:** the exact argv or key sequence (log a short `proof.md`).
2. **Visible result:** CLI stdout or tmux pane capture showing the user-visible end state.
3. **Side effect:** listing of `$GROG_HOME` after the drive, plus a check that `~/.grok` was not created by the run (or mtime unchanged if it already existed).
4. **Doctor:** `scripts/doctor.sh` output from before the drive.

Example layout:

```
.cursor/skills/verify-grog/artifacts/
  proof.md
  version.txt
  doctor-cli.txt
  doctor-script.txt
  grog-home-listing.txt
  official-grok-home.txt
  help-palette.txt          # if the driven feature was /help
```

Exercise the real user path. Do not call internal setters, do not POST test-only mock endpoints, do not treat unit-test goldens as a user proof.

## Cleanup

```sh
.cursor/skills/verify-grog/scripts/cleanup.sh
```

- Kills tmux session `grog-verify-$VERIFY_RUN_ID` only (the session this run started).
- Deletes `$GROG_HOME` only if `.verify-grog-owned` is present.
- Leaves `artifacts/` in place.

After cleanup, confirm artifacts still exist. A cleanup that eats the proof fails verification.

On a failed iteration, run cleanup before the next launch with the same `VERIFY_RUN_ID`.

## Helpers

All scripts are executable. Source `common.sh` only from the others.

```sh
.cursor/skills/verify-grog/scripts/launch.sh
.cursor/skills/verify-grog/scripts/doctor.sh
.cursor/skills/verify-grog/scripts/drive-tmux.sh start|wait|type|enter|keys|capture|quit|pane
.cursor/skills/verify-grog/scripts/cleanup.sh
```

Env knobs: `VERIFY_RUN_ID`, `VERIFY_GROG_HOME` (must stay `/tmp/grog-verify-*`), `GROG_BIN`, `PROTOC`, `VERIFY_ARTIFACTS`.

## Hard rules

- Never create or write `~/.grok`. Never overwrite `~/.grog` belonging to a human. Never touch `~/.codex`, `~/.claude`, or Cursor agent paths except read-only doctor/login probes the feature file names.
- Never `pkill -f grog` / `pkill grok`.
- Refuse to drive a shared instance (existing `~/.grog`, existing leader.sock you did not start, tmux session whose name you did not allocate).
- Skip `/council` live seats, `/update` GitHub Darwin install, and Ask\* consults when vendor CLIs/tokens are missing. Document the skip; do not fake a pass.
- Linux cargo only. Do not compile grog on Darwin. Do not run `grog update` against this Linux proof unless the feature file is explicitly testing the skip/error path.

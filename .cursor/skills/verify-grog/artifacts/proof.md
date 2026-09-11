# Proof: dump-rebase c4ea71cf (grog 1.0.24)

Driven per `.cursor/skills/verify-grog/features/identity-and-home.md`, `doctor.md`, and `SKILL.md` after rebasing onto xai-org/grok-build `37949780` (SOURCE_REV `c4ea71cfdbcdb21e32e41bc25a0043d7d4836714`).

## Action

```
VERIFY_RUN_ID=proof-c4ea71cf
scripts/launch.sh
scripts/doctor.sh
GROG_HOME=/tmp/grog-verify-proof-c4ea71cf
unset GROK_HOME
target/debug/grog --version
target/debug/grog doctor
target/debug/grog doctor --json
scripts/drive-tmux.sh start
scripts/drive-tmux.sh wait 'Waiting for approval' 25
scripts/cleanup.sh
```

## Visible result

- `version.txt`: `grog 1.0.24 (9b25810e0ad6)` — product name is grog, not grok.
- `doctor-script.txt`: identity grog; GROG_HOME owned by this run; official `~/.grok` absent.
- `doctor-cli.txt`: `Grok Doctor` (dump terminal block) plus `Grog providers` (`claude-bridge` / `antigravity` / `codex` all `missing` on this VM) and Privacy defaults (telemetry off, marketplace empty, feedback off).
- `doctor-json.txt`: dump `grog doctor --json` (`schemaVersion` `"1"`). Grog providers stay on the human path, not this JSON blob.
- `tui-login-gate.txt`: real pager TUI under isolated home; login gate (`Waiting for approval...`, `ctrl+q  quit`). No browser login was completed.

## Side effects

- `grog-home-listing.txt`: writes only under `/tmp/grog-verify-proof-c4ea71cf`.
- `official-grok-home.txt`: `official ~/.grok still absent`.
- `user-grog-home.txt`: pre-existing user `~/.grog` was not selected (GROG_HOME override); grog did not use `~/.grok`.

## Cleanup

`scripts/cleanup.sh` removed `/tmp/grog-verify-proof-c4ea71cf` and the tmux session. These artifact files remained.

## Skips (not passes)

- `skip-help.md` — `/help` on the login gate does not open the command palette (`New Session` absent).
- `skip-council.md` — no Claude / agy / Codex credentials; live `/council` deliberation skipped.
- `skip-advisor.md` — no agent session (login gate); consults would also need a seat.
- Live `grog update` / Darwin compile — out of scope on this Linux host; Mac is download-only via macos-14.
- Isolation unit test (not a live GitHub install): `cargo test -p xai-grok-pager-pty-harness --test update_never_blocked_by_config` passed. Dump moved this spawn test out of pager-bin.

# Proof: dump-rebase be7ce6e8 (grog 1.0.32)

Driven per `.cursor/skills/verify-grog/features/identity-and-home.md`, `doctor.md`, and `SKILL.md` after rebasing onto xai-org/grok-build `48271133` (SOURCE_REV `be7ce6e8cffe46d20bef9834b211616082ee866b`).

## Action

```
VERIFY_RUN_ID=proof-be7ce6e8
scripts/launch.sh
scripts/doctor.sh
GROG_HOME=/tmp/grog-verify-proof-be7ce6e8
unset GROK_HOME
target/debug/grog --version
target/debug/grog doctor
target/debug/grog doctor --json
scripts/drive-tmux.sh start
scripts/drive-tmux.sh wait 'Waiting for approval' 25
scripts/cleanup.sh
```

## Visible result

- `version.txt`: `grog 1.0.32 (6db81df84894)` — product name is grog, not grok.
- `doctor-script.txt`: identity grog; GROG_HOME owned by this run; official `~/.grok` absent.
- `doctor-cli.txt`: `Grok Doctor` (dump terminal block) plus `Grog providers` (`claude-bridge` / `antigravity` / `codex` all `missing` on this VM) and Privacy defaults (telemetry off, marketplace empty, feedback off).
- `doctor-json.txt`: dump `grog doctor --json` (`schemaVersion` `"1"`). Grog providers stay on the human path, not this JSON blob.
- `tui-login-gate.txt`: real pager TUI under isolated home; login gate (`Waiting for approval...`, `ctrl+q  quit`). No browser login was completed.

## Side effects

- `grog-home-listing.txt`: writes only under `/tmp/grog-verify-proof-be7ce6e8`.
- `official-grok-home.txt`: `official ~/.grok still absent`.
- `user-grog-home.txt`: pre-existing user `~/.grog` was not selected (GROG_HOME override); grog did not use `~/.grok`.

## Cleanup

`scripts/cleanup.sh` removed `/tmp/grog-verify-proof-be7ce6e8` and the tmux session. These artifact files remained.

## Skips (not passes)

- `skip-help.md` — `/help` on the login gate does not open the command palette (`New Session` absent).
- `skip-council.md` — no Claude / agy / Codex credentials; live `/council` deliberation skipped.
- `skip-advisor.md` — no agent session (login gate); consults would also need a seat.
- Live `grog update` / Darwin compile — out of scope on this Linux host; Mac is download-only via macos-14.
- Isolation unit test (not a live GitHub install): `cargo test -p xai-grok-pager-pty-harness --test update_never_blocked_by_config` passed.

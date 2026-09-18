# Proof: dump-rebase e8563f8f (grog 1.0.35)

Driven per `.cursor/skills/verify-grog/features/identity-and-home.md`, `doctor.md`, and `SKILL.md` after rebasing onto xai-org/grok-build `a28ee2b` (SOURCE_REV `e8563f8f182296ebb53cadb3e1eab7615d76408e`).

## Action

```
VERIFY_RUN_ID=proof-e8563f8f
scripts/launch.sh
scripts/doctor.sh
GROG_HOME=/tmp/grog-verify-proof-e8563f8f
unset GROK_HOME
target/debug/grog --version
target/debug/grog doctor
target/debug/grog doctor --json
scripts/drive-tmux.sh start
scripts/drive-tmux.sh wait 'Waiting for approval' 25
scripts/cleanup.sh
```

## Visible result

- `version.txt`: `grog 1.0.35 (cf0e825b3e50)` — product name is grog, not grok.
- `doctor-script.txt`: identity grog; GROG_HOME owned by this run; official `~/.grok` absent.
- `doctor-cli.txt`: `Grok Doctor` (dump terminal block) plus `Grog providers` (`claude-bridge` / `antigravity` / `codex` all `missing` on this VM) and Privacy defaults (telemetry off, marketplace empty, feedback off).
- `doctor-json.txt`: dump `grog doctor --json` (`schemaVersion` `"1"`). Grog providers stay on the human path, not this JSON blob.
- `tui-login-gate.txt`: real pager TUI under isolated home; login gate (`Waiting for approval...`, `ctrl+q  quit`). No browser login was completed.

## Side effects

- `grog-home-listing.txt`: writes only under `/tmp/grog-verify-proof-e8563f8f`.
- `official-grok-home.txt`: `official ~/.grok still absent`.
- `user-grog-home.txt`: pre-existing user `~/.grog` was not selected (GROG_HOME override); grog did not use `~/.grok`.

## Cleanup

`scripts/cleanup.sh` removed `/tmp/grog-verify-proof-e8563f8f` and the tmux session. These artifact files remained.

## Skips (not passes)

- `skip-help.md` — `/help` on the login gate does not open the command palette (`New Session` absent).
- `skip-council.md` — no Claude / agy / Codex credentials; live `/council` deliberation skipped.
- `skip-advisor.md` — no agent session (login gate); consults would also need a seat.
- Live `grog update` / Darwin compile — out of scope on this Linux host; Mac is download-only via macos-14.
- Isolation unit test (not a live GitHub install): `cargo test -p xai-grok-pager-pty-harness --test update_never_blocked_by_config` passed.

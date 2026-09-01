# Proof: identity-and-home

Driven per `.cursor/skills/verify-grog/features/identity-and-home.md` and `SKILL.md`.

## Action

```
VERIFY_RUN_ID=proof-identity
scripts/launch.sh
scripts/doctor.sh
GROG_HOME=/tmp/grog-verify-proof-identity
unset GROK_HOME
target/debug/grog --version
target/debug/grog doctor
```

TUI follow-up (same isolation, later run `proof-help`): `scripts/drive-tmux.sh start` then wait for `Waiting for approval`.

## Visible result

- `version.txt`: `grog 1.0.13 (ae52579527bf)` — product name is grog, not grok.
- `doctor-cli.txt`: `Grok Doctor` (terminal block) plus `Grog providers` (`claude-bridge` / `antigravity` / `codex` all `missing` on this VM) and Privacy defaults.
- `tui-login-gate.txt`: real pager TUI under isolated home; login gate (`Waiting for approval...`, `ctrl+q  quit`). No browser login was completed.

## Side effects

- `grog-home-listing.txt`: writes only under `/tmp/grog-verify-proof-identity`.
- `official-grok-home.txt`: `official ~/.grok still absent`.
- `user-grog-home.txt`: user `~/.grog` still absent.

## Cleanup

`scripts/cleanup.sh` removed `/tmp/grog-verify-proof-identity` and the tmux session. These artifact files remained.

## Skips (not passes)

- `skip-help.md` — `/help` on the login gate did not open the command palette (`New Session` absent).
- `skip-council.md` — no Claude / agy / Codex credentials.
- `skip-advisor.md` — no agent session (login gate); consults would also need a seat.

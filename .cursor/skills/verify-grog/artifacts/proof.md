# Proof: dump-rebase 036a5d83 (grog 1.0.41)

Driven per `.cursor/skills/verify-grog/features/identity-and-home.md`, `doctor.md`, and `SKILL.md` after rebasing onto xai-org/grok-build `f0e3be11` (SOURCE_REV `036a5d8348cd744767cd0b08518ab17bf608fa7f`).

## Action

```
VERIFY_RUN_ID=proof-036a5d83
scripts/launch.sh
scripts/doctor.sh
GROG_HOME=/tmp/grog-verify-proof-036a5d83
unset GROK_HOME
target/debug/grog --version
target/debug/grog doctor
target/debug/grog doctor --json
target/debug/grog models
scripts/drive-tmux.sh start
scripts/drive-tmux.sh wait 'Waiting for approval' 25
scripts/cleanup.sh
```

CLI isolation re-check (exported `GROG_HOME`): `grog doctor` under `/tmp/grog-verify-proof-036a5d83-cli`; user `~/.grog` mtime unchanged.

## Visible result

- `version.txt`: `grog 1.0.41 (ccba87a98049)` — product name is grog, not grok.
- `doctor-script.txt`: identity grog; GROG_HOME owned by this run; official `~/.grok` absent.
- `doctor-cli.txt`: `Grok Doctor` (dump terminal block) plus `Grog providers` (`claude-bridge` / `antigravity` / `codex` all `missing` on this VM) and Privacy defaults (telemetry off, marketplace empty, feedback off).
- `doctor-json.txt`: dump `grog doctor --json` (`schemaVersion` `"1"`). Grog providers stay on the human path, not this JSON blob.
- `models-cli.txt`: catalog lists `codex/`, `claude-bridge/`, and `antigravity/` ids (native providers).
- `tui-login-gate.txt`: real pager TUI under isolated home; login gate (`Waiting for approval...`, `ctrl+q  quit`). No browser login was completed.
- Isolated `config.toml` after TUI start had only `[marketplace] default_skills_installs_purged = true`. No vendor tokens.

## Side effects

- `grog-home-listing.txt`: TUI writes only under `/tmp/grog-verify-proof-036a5d83`.
- `official-grok-home.txt`: `official ~/.grok still absent`.
- `user-grog-home.txt`: user `~/.grog` was not selected when `GROG_HOME` is exported. Official `~/.grok` was never created.

## Cleanup

`scripts/cleanup.sh` removed `/tmp/grog-verify-proof-036a5d83` and the tmux session. These artifact files remained.

## Skips (not passes)

- `skip-help.md` — `/help` on the login gate does not open the command palette (`New Session` absent).
- `skip-council.md` — no Claude / agy / Codex credentials; live `/council` deliberation skipped.
- `skip-advisor.md` — no agent session (login gate); consults would also need a seat.
- Live `grog update` / Darwin compile — out of scope on this Linux host; Mac is download-only via macos-14.
- Isolation unit test (not a live GitHub install): `cargo test -p xai-grok-pager-pty-harness --test update_never_blocked_by_config` passed after installing `libssl-dev` on this VM.

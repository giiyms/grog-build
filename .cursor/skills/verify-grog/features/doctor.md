# Doctor

Doctor is grog's "is this environment usable?" report. From the shell, `grog doctor` prints terminal/clipboard/color diagnostics under the heading **Grok Doctor**, then a **Grog providers** block (`claude-bridge`, `antigravity`, `codex`) and Privacy defaults. Inside the TUI, `/doctor` (aliases `/terminal-setup`, `/terminal-check`, `/terminal-info`) diagnoses the live session and can offer `/doctor fix …` automatic fixes. Missing provider binaries are reported as `missing`, not a crash.

## Sub-features

- CLI: `grog doctor` and `grog doctor --json`.
- CLI: `grog doctor fix [id] [--yes]` for automatic terminal fixes (interactive confirm unless `--yes`).
- TUI: `/doctor` session report; `/doctor fix` lists fixes.
- Provider lines: PATH for `claude` / `agy`, Codex tokens at `$GROG_HOME/auth/codex.json` or hint to import `~/.codex/auth.json`.

## How to get to it (user POV)

```sh
grog doctor
```

In the TUI composer: `/doctor`.

## Driving it with tmux/PTY

CLI is the proof path (no TUI required):

```sh
export VERIFY_RUN_ID=proof-doctor
.cursor/skills/verify-grog/scripts/launch.sh
.cursor/skills/verify-grog/scripts/doctor.sh
GROG_HOME=/tmp/grog-verify-$VERIFY_RUN_ID
unset GROK_HOME
target/debug/grog doctor | tee .cursor/skills/verify-grog/artifacts/doctor-cli.txt
```

Pass: output contains `Grok Doctor` and `Grog providers`, and names `claude-bridge`, `antigravity`, and `codex`. A `missing` mark for those providers is a valid pass on a VM without vendor CLIs. `$GROG_HOME` may be created as a side effect of home resolution; `~/.grok` must not appear.

TUI optional: `drive-tmux.sh start`, wait for `Quit`, type `/doctor`, Enter, capture a pane that is not the untouched welcome screen.

## Gotchas

- Heading **Grok Doctor** is dump terminal copy; the grog-specific section is **Grog providers**. Do not fail the proof because the first heading still says Grok.
- `grog doctor fix` can edit the user's shell rc if you pass `--yes`. Do not apply fixes in verification.
- Skill `scripts/doctor.sh` is the instance health check; `grog doctor` is the product command. Run both when proving this feature.
- Codex doctor reads `$GROG_HOME` then `~/.codex/auth.json` (read-only). It must not copy tokens unless the user ran `grog login codex`.

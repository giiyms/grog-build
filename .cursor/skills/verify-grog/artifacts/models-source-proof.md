# grog models source, Gemini 3.8, hide

## Action
- `VERIFY_RUN_ID=models-source-19` `.cursor/skills/verify-grog/scripts/launch.sh`
- `GROG_HOME=/tmp/grog-verify-models-source-19` `target/debug/grog --version`
- `grog models` unauthenticated
- write `$GROG_HOME/hidden-models` with `antigravity/gemini-3.6-flash`, run `grog models` again
- delete that file, run `grog models` again

## Visible result
- `--version` is `grog 1.0.32 (23be1fc8589a)`
- each catalog row prints `id  source` (Grok, Codex, Claude, Antigravity)
- Antigravity includes `gemini-3.8-flash-high`, `gemini-3.8-flash-medium`, `gemini-3.8-flash-low`
- hidden `antigravity/gemini-3.6-flash` is omitted. `gemini-3.6-flash-high` stays.
- after deleting `hidden-models`, `gemini-3.6-flash` returns

## Side effect
- `$GROG_HOME` is `/tmp/grog-verify-models-source-19` with `.verify-grog-owned`
- `/home/ubuntu/.grok` is absent

## TUI
Skipped. No vendor login, pager stops at the login gate.

## Doctor
`scripts/doctor.sh` READY. Identity grog, home owned by this run.

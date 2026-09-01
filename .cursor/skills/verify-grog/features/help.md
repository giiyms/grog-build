# Help

Typing `/help` in the pager opens the command palette — the searchable browser of slash commands and keyboard shortcuts. It is the same overlay as `Ctrl+P`. In `--minimal` mode the idle status line advertises `/help` because there is no fullscreen footer. The palette lists entries such as **New Session**; Esc dismisses it back to the prompt or welcome composer.

## Sub-features

- `/help` — open the palette (works without an agent session).
- `Ctrl+P` — same palette.
- Type-to-filter inside the palette; Enter runs the selected command.
- Esc — leave input mode / close the palette (may take two Esc if input is focused).

## How to get to it (user POV)

1. Start grog (welcome screen is enough; a live model is not required).
2. Focus the composer.
3. Type `/help` and press Enter, or press `Ctrl+P`.
4. Confirm the palette is showing commands (look for **New Session**).
5. Esc until the overlay is gone.

## Driving it with tmux/PTY

```sh
export VERIFY_RUN_ID=proof-help
.cursor/skills/verify-grog/scripts/launch.sh
.cursor/skills/verify-grog/scripts/doctor.sh
.cursor/skills/verify-grog/scripts/drive-tmux.sh start
# Welcome: Quit. Login gate (no creds): Waiting for approval / ctrl+q  quit.
.cursor/skills/verify-grog/scripts/drive-tmux.sh wait 'Waiting for approval' 25 \
  || .cursor/skills/verify-grog/scripts/drive-tmux.sh wait 'Quit' 5
.cursor/skills/verify-grog/scripts/drive-tmux.sh capture help-before.txt
.cursor/skills/verify-grog/scripts/drive-tmux.sh type '/help'
.cursor/skills/verify-grog/scripts/drive-tmux.sh enter
.cursor/skills/verify-grog/scripts/drive-tmux.sh wait 'New Session' 15
.cursor/skills/verify-grog/scripts/drive-tmux.sh capture help-palette.txt
.cursor/skills/verify-grog/scripts/drive-tmux.sh quit || true
.cursor/skills/verify-grog/scripts/cleanup.sh
```

Pass: `help-palette.txt` contains `New Session` (palette-only; not the welcome `Quit` menu alone). Side effect: `$GROG_HOME` may gain config/session scaffolding; `~/.grok` must still be untouched.

If the pane is the login gate, `/help` may be ignored (keys belong to the auth UI). Skip `/help` with that precondition and prove [identity-and-home](identity-and-home.md). Do not complete browser login or inject a real xAI key unless the user asked.

## Gotchas

- The slash dropdown while typing `/he` is not the palette. Submit with Enter.
- `New Session` is the stable palette sentinel used by `minimal_help_opens_command_palette`.
- Welcome `Quit` is case-sensitive; authenticating uses lowercase `quit`.
- `/help` does not need `/council` seats or API keys.

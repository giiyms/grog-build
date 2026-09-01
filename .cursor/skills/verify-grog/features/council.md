# Council

`/council` runs a Karpathy-style deliberation: several models answer independently, then (when more than one seat answered) they rank anonymized copies, then a chair writes one verdict. The user types `/council <query>` in the TUI (or `/workflow council …` / `grog -p --workflow council`). Default seats are Claude Code, Antigravity/agy, and ChatGPT Codex. The visible product is the independent answers plus ranking plus chair note — never a null report. Nested Ask*/council recursion is denied.

## Sub-features

- `/council <query>` — first-class slash alias (same gate as `/deep-research`).
- `/workflow council {"query":"…","members":[…],"apply":false}` — same workflow with overrides.
- Degraded membership: if only one seat starts, review is skipped but the seat's answer and a chair note still appear.
- `apply: true` lets the chair write; default consults are read-only.

## How to get to it (user POV)

In an authenticated grog session with provider logins:

```
/council implement the cache invalidation
```

Requires at least one of: `claude` on PATH (Claude Code login), `agy` on PATH (Google login), or Codex tokens (`grog login codex` / `~/.grog/auth/codex.json`).

## Driving it with tmux/PTY

```sh
# Only after grog doctor shows at least one provider ok.
.cursor/skills/verify-grog/scripts/drive-tmux.sh start
.cursor/skills/verify-grog/scripts/drive-tmux.sh wait 'Quit' 25
# Promote to a session if still on welcome (user would type a prompt or pick New Session).
.cursor/skills/verify-grog/scripts/drive-tmux.sh type '/council what is 2+2'
.cursor/skills/verify-grog/scripts/drive-tmux.sh enter
# Wait for a council report in scrollback (opinions / ranking / verdict), not a spinner forever.
.cursor/skills/verify-grog/scripts/drive-tmux.sh capture council-report.txt
```

**Skip on this VM without vendor credentials.** Unmet precondition: no `claude`, no `agy`, no Codex tokens under the disposable `GROG_HOME`. Write `artifacts/skip-council.md`. Do not fake a report. Do not paste a parent grok token to force HTTP through `grog://` (that path is a known spawn-death).

Pass (when creds exist): pane or session log under `$GROG_HOME/sessions/` contains the query and a chair synthesis; failed seats are named rather than a blank reply.

## Gotchas

- Council is session-scoped. Welcome-only `/council` may not launch until an agent session exists.
- Default Codex id is `codex/gpt-5.6-luna`, not `gpt-5.3-codex` (consumer ChatGPT rejects the latter).
- Live council spends real vendor quota. Keep the query tiny.
- Linux proofs cannot use Mac-only install paths. Provider CLIs must already be on PATH.

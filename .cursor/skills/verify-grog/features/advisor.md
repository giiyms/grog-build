# Advisor

`/advisor` is a session-scoped sidecar that watches the live primary transcript and may inject at most one note per update (default SILENCE). It is not council and not a second full agent turn. Enable/disable lasts for this session only. The advisor **model** is persisted in config (`models.advisor`) so picking Luna for advisor does not move the live session off its primary model. `/advisor status` shows the pretty name and qualified id.

## Sub-features

- `/advisor` — toggle enable for this session.
- `/advisor on` / `/advisor off` — explicit enable/disable (`off` keeps `models.advisor`).
- `/advisor status` — pretty name + qualified id vs config.
- `/advisor model` or `/advisor luna` (also `opus`, `sonnet`, `agy`) — persist slot and enable.
- Tab on an empty query, or Settings **Advisor model** row — picker targeting advisor, not the session.

## How to get to it (user POV)

In a running session:

```
/advisor status
/advisor on
/advisor off
```

To pick a model without switching the primary: `/advisor model` or `/settings` → Advisor model.

## Driving it with tmux/PTY

Status/toggle can be proven **without** vendor consults once a session exists:

```sh
.cursor/skills/verify-grog/scripts/drive-tmux.sh start
.cursor/skills/verify-grog/scripts/drive-tmux.sh wait 'Quit' 25
# If still on welcome, /advisor is session-scoped and should error or be hidden.
.cursor/skills/verify-grog/scripts/drive-tmux.sh type '/advisor status'
.cursor/skills/verify-grog/scripts/drive-tmux.sh enter
.cursor/skills/verify-grog/scripts/drive-tmux.sh capture advisor-status.txt
```

Pass (session present): pane shows advisor status (name/id or disabled), not a crash. Config side effect for `/advisor luna`: `$GROG_HOME/config.toml` gains `models.advisor` without changing `models.default`.

**Skip consult proof** when no Codex/Claude/agy seat can actually answer. Enabling the sidecar without a working model is not a fake pass of "advisor injected a note".

If welcome blocks session creation (no login), skip with that precondition and prove [help](help.md) or [identity-and-home](identity-and-home.md) instead.

## Gotchas

- `/advisor` on welcome: session-scoped; expect an error like no active session rather than a consult.
- Picking an advisor model must not change the live `/model`.
- Consults go through native providers (no `grog://` HTTP). Without tokens the sidecar stays silent — that is not a pass for injection.
- `/restart` restores the session; advisor enable is session-scoped and does not persist across restart unless you re-enable (model id in config does persist).

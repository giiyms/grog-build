# Updating grog from upstream Grok Build

This tree is periodically dumped from the SpaceXAI monorepo. `SOURCE_REV` at
the repo root is the monorepo commit of the last dump. `main` commits titled
`Synced from monorepo` are those dumps.

Grog stays easy to rebase if grog-specific code stays out of the dump's
renames and stays in a few places.

## Do not

- Rename `xai-grok-*` crates, Bazel paths, or the `xai-` prefix.
- Copy vendor tokens (Anthropic, Google, ChatGPT) into `config.toml`.
- Wrap the pi npm packages (`pi-claude-bridge`, `@estebanforge/pi-antigravity-bridge`).
- Scrape hidden vendor APIs when a supported CLI or official OAuth exists.

## Keep grog here

| Kind | Where |
| --- | --- |
| Native providers | `crates/codegen/grog-*` (`grog-providers`, `grog-claude-bridge`, `grog-antigravity`, `grog-codex`) |
| Council workflow | `crates/codegen/xai-grok-shell/src/session/workflows/council.rhai` |
| Thin identity patches | pager bin name, `~/.grog` / `$GROG_HOME`, prompts saying **Grog**, privacy defaults, `/council` slash alias, native-turn intercept (qualified `codex/` / `claude-bridge/` / `antigravity/` ids, never POST `grog://`), **no x.ai/cli updater** (`grog --version`, skip grok update toast/changelog CDN) |
| Product docs | `docs/grog.md`, this file |

Thin patches should stay small so the next dump rebases. Prefer a new grog
crate over editing a large upstream file when both work.

## Rebase onto the next dump

```sh
git fetch origin main
git rebase origin/main   # on your grog branch
# if SOURCE_REV moved, leave it as the dump's SHA
```

Conflicts usually land in:

- `xai-grok-pager-bin` / pager clap argv0
- `xai-grok-home` (`GROG_HOME`)
- `xai-grok-shell` catalog merge, sampler intercept, slash commands, privacy
- `xai-grok-agent/templates/*.md` (re-run `python3 scripts/encrypt_templates.py`
  from `crates/codegen/xai-grok-agent` after resolving prompt.md)

After a dump, `cargo test -p grog-providers -p grog-claude-bridge -p grog-antigravity -p grog-codex`
and `cargo test -p xai-grok-shell --lib council_fans_out -- council_projects -- degraded_membership`
plus `cargo check -p xai-grok-pager-bin`.

## Build and install grog (not the x.ai installer)

`install.sh` still ships official `grok` from x.ai/cli. This fork is from source:

```sh
export PATH="$HOME/.local/bin:$PATH"
# protoc: repo `bin/protoc` via DotSlash, or `PROTOC=$(which protoc)`
cargo build -p xai-grok-pager-bin --release
install -m 755 target/release/xai-grok-pager "$HOME/.local/bin/grog"
grog --version
grog doctor
```

`grog --version` prints `grog …`. Grog does **not** check x.ai/cli for grok
upgrades and does not show the official Grok Build “update available”
banner. Rebuild from source to update grog. Official `grok`
(`~/.grok/bin/grok`) is a separate binary.

`grok` remains a valid argv0 alias during the transition.

## Providers and ToS

Claude and Antigravity are **CLI bridges**. Grog spawns the user's `claude` /
`agy`; grog does not hold those subscriptions. Codex is **ChatGPT OAuth**
imported from `~/.codex/auth.json` (same public client id and
`chatgpt.com/backend-api` the official Codex CLI uses), not `OPENAI_API_KEY`.

See [grog.md](grog.md) for the council workflow and provider catalog.

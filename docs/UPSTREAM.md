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
| Native providers | `crates/codegen/grog-*` (`grog-providers`, `grog-claude-bridge`, `grog-antigravity`, `grog-codex`, `grog-update`) |
| Advisor sidecar | `crates/codegen/grog-advisor` (`/advisor`; picker target + `models.advisor`; not a pi wrap) |
| Council workflow | `crates/codegen/xai-grok-shell/src/session/workflows/council.rhai` |
| Thin identity patches | pager bin name, `~/.grog` / `$GROG_HOME`, prompts saying **Grog**, privacy defaults, `/council` slash alias, native-turn intercept (qualified `codex/` / `claude-bridge/` / `antigravity/` ids, never POST `grog://`), title-gen skip for native models, **no x.ai/cli updater** (`grog --version`, skip grok update toast/changelog CDN), GitHub Releases `grog update` / `/update` (`grog-update`) |
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
- `xai-dirs` (`GROG_HOME` / `~/.grog`; dump's single home resolver. Official grok stays in `~/.grok`)
- `xai-grok-shell` catalog merge, sampler intercept, slash commands, privacy
- pager TUI `/update` `/restart` / Session vs Advisor picker (dump moved `input/` and `search/` into `xai-grok-pager-render`)
- `xai-grok-agent/templates/*.md` (re-run `python3 scripts/encrypt_templates.py`
  from `crates/codegen/xai-grok-agent` after resolving prompt.md)

After a dump, `cargo test -p grog-providers -p grog-claude-bridge -p grog-antigravity -p grog-codex -p grog-advisor -p grog-update`
and `cargo test -p xai-grok-shell --lib council_fans_out -- council_projects -- degraded_membership`
plus `cargo check -p xai-grok-pager-bin`.

## Install grog (not the x.ai installer)

`install.sh` still ships official `grok` from x.ai/cli. This fork’s ship path is
the GitHub Actions `grog macOS aarch64` workflow (native `macos-14` arm64). Do
not compile grog on a disk-constrained Mac. The Mac is **download-only**.

After a green run on `main`:

1. `grog update` — fetches the rolling prerelease
   [grog-macos-aarch64](https://github.com/giiyms/grog-build/releases/tag/grog-macos-aarch64)
   via the public GitHub Releases API, verifies the `.sha256` sidecar when
   present, and installs:
   - `~/.grog/downloads/grog-<ver>-macos-aarch64`
   - symlink `~/.grog/bin/grog`
   - symlink `~/.local/bin/grog`
2. Or download the Actions artifact / rolling asset by hand and lay it out
   the same way.

`grog --version` prints `grog <Cargo.toml version> (<commit>)`. Grog does
**not** check x.ai/cli for grok upgrades and does not show the official Grok
Build “update available” banner. Official `grok` (`~/.grok/bin/grok`) is a
separate binary and is never overwritten.

In the TUI, `/update` (alias `/upgrade`) is the same install as `grog update`,
then `/restart` (resume this session on the new `~/.grog/bin/grog`).

`grok` remains a valid argv0 alias during the transition.

## Cutting a grog version

The crate version **is** the release version. Keep these lockstepped:

- `crates/codegen/xai-grok-pager-bin/Cargo.toml`
- `crates/codegen/xai-grok-version/Cargo.toml`
- `crates/codegen/xai-grok-pager/Cargo.toml`
- `crates/codegen/xai-grok-shell/Cargo.toml`

To cut `1.0.10` (example):

1. Bump those four `version =` fields to `1.0.10`.
2. Merge to `main`.
3. The macos-14 workflow:
   - always refreshes the rolling prerelease tag `grog-macos-aarch64` (the
     one URL `grog update` follows);
   - creates GitHub Release `v1.0.10` **only if that tag does not already
     exist**. Later `1.0.10` commits on main still update rolling, and do
     not pile more versioned releases.

Do not open a versioned GitHub Release from a machine that compiled grog.
The only Darwin aarch64 artifact is the GitHub-hosted `macos-14` job.

## Providers and ToS

Claude and Antigravity are **CLI bridges**. Grog spawns the user's `claude` /
`agy`; grog does not hold those subscriptions. Codex is **ChatGPT OAuth**
imported from `~/.codex/auth.json` (same public client id and
`chatgpt.com/backend-api` the official Codex CLI uses), not `OPENAI_API_KEY`.

See [grog.md](grog.md) for the council workflow and provider catalog.

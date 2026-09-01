# Identity and home

Grog identifies itself as **grog**, not official xAI **grok**. `grog --version` prints a line starting with `grog`. Data lives under `$GROG_HOME` (then `$GROK_HOME`) or `~/.grog`. Official grok's `~/.grok` is a different product tree: grog must not create or write it. argv0 `grok` or `agent` still runs this binary; resume hints and `/restart` copy still say `grog`.

## Sub-features

- Version string: `grog <version>` (`grog --version` / `-v` / `-V`).
- Home resolver: `GROG_HOME` wins; else `GROK_HOME`; else `<home>/.grog`.
- Isolation: a disposable `GROG_HOME` receives sessions, config, auth, leader socket; `~/.grok` stays untouched.
- CLI name aliases: argv0 `grog` | `grok` | `agent` parse as this app; user-facing resume text is `grog --continue`.

## How to get to it (user POV)

From a shell in the repo (or any install that puts `grog` on `PATH`):

```sh
grog --version
grog doctor
```

First interactive launch without `GROG_HOME` would use `~/.grog`. Verification never does that.

## Driving it with tmux/PTY

tmux is optional. Prove this feature with the CLI under the skill isolation env:

```sh
export VERIFY_RUN_ID=proof-identity
.cursor/skills/verify-grog/scripts/launch.sh
.cursor/skills/verify-grog/scripts/doctor.sh
GROG_HOME=/tmp/grog-verify-$VERIFY_RUN_ID
unset GROK_HOME
target/debug/grog --version | tee .cursor/skills/verify-grog/artifacts/version.txt
target/debug/grog doctor    | tee .cursor/skills/verify-grog/artifacts/doctor-cli.txt
find "$GROG_HOME" -maxdepth 3 | tee .cursor/skills/verify-grog/artifacts/grog-home-listing.txt
test ! -e "$HOME/.grok" && echo "official ~/.grok still absent" \
  | tee .cursor/skills/verify-grog/artifacts/official-grok-home.txt
.cursor/skills/verify-grog/scripts/cleanup.sh
test -s .cursor/skills/verify-grog/artifacts/version.txt
```

Pass: stdout starts with `grog `; `$GROG_HOME` exists and is not `~/.grok`; after the drive `~/.grok` is still absent (or mtime unchanged if a human already had it); cleanup removes `$GROG_HOME` but leaves `artifacts/`.

## Gotchas

- `GROK_HOME` leftover in the environment **overrides** the default `~/.grog` and can point at official grok. Always unset it for verification.
- Dump comments still mention `~/.grok/leader.sock`. Isolation is the env + `--no-leader`, not those comments.
- `xai-grok-pager-pty-harness` tests seed `GROK_HOME` under a fake `$HOME/.grok`. That is the test sandbox, not the grog product default. Do not copy that layout for live proofs.
- macOS install/`grog update` writes `~/.grog/bin/grog`. Linux cargo proofs must not run `grog update` expecting a Darwin artifact.

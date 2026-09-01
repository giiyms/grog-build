#!/usr/bin/env bash
# Tear down instances this run created. Evidence in artifacts/ is kept.
set -euo pipefail
# shellcheck source=common.sh
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/common.sh"

echo "verify-grog cleanup"
echo "  session:   ${VERIFY_TMUX_SESSION}"
echo "  GROG_HOME: ${VERIFY_GROG_HOME}"
echo "  artifacts: ${VERIFY_ARTIFACTS} (kept)"

if verify_tmux has-session -t "=${VERIFY_TMUX_SESSION}" 2>/dev/null; then
  # Kill the session we started (tmux reaps the pane process). Never pkill grog.
  verify_tmux kill-session -t "=${VERIFY_TMUX_SESSION}"
  echo "OK  killed tmux session ${VERIFY_TMUX_SESSION}"
else
  echo "INFO no tmux session ${VERIFY_TMUX_SESSION}"
fi

if [[ -f "${VERIFY_MARKER}" ]]; then
  rm -rf "${VERIFY_GROG_HOME}"
  echo "OK  removed ${VERIFY_GROG_HOME}"
elif [[ -d "${VERIFY_GROG_HOME}" ]]; then
  echo "REFUSING to delete ${VERIFY_GROG_HOME} — missing ${VERIFY_MARKER}"
  echo "This home was not tagged as a verify-grog instance."
  exit 2
else
  echo "INFO GROG_HOME already gone"
fi

echo "verify-grog cleanup: done. artifacts remain at ${VERIFY_ARTIFACTS}"
ls -la "${VERIFY_ARTIFACTS}" 2>/dev/null || echo "(no artifacts dir yet)"

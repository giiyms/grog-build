#!/usr/bin/env bash
# Build grog once. Does not keep a TUI running — each drive starts its own PTY.
set -euo pipefail
# shellcheck source=common.sh
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/common.sh"

verify_ensure_protoc
verify_ensure_home
verify_export_isolation

echo "verify-grog launch"
echo "  repo:     ${VERIFY_REPO_ROOT}"
echo "  bin:      ${VERIFY_GROG_BIN}"
echo "  GROG_HOME:${VERIFY_GROG_HOME}"
echo "  PROTOC:   ${PROTOC:-<(bin/protoc or PATH)}"
echo "  artifacts:${VERIFY_ARTIFACTS}"

cd "${VERIFY_REPO_ROOT}"
cargo build -p xai-grok-pager-bin --bin grog

if [[ ! -x "${VERIFY_GROG_BIN}" ]]; then
  echo "verify-grog: expected binary missing at ${VERIFY_GROG_BIN}" >&2
  exit 1
fi

echo "verify-grog: ready. Next: scripts/doctor.sh, then a drive in its own tmux/PTY."
echo "verify-grog: TUI start example:"
echo "  GROG_HOME=${VERIFY_GROG_HOME} ${VERIFY_GROG_BIN} --no-leader"
echo "verify-grog: or: ${VERIFY_SKILL_DIR}/scripts/drive-tmux.sh start"

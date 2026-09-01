#!/usr/bin/env bash
# Read-only: is this instance worth driving?
set -euo pipefail
# shellcheck source=common.sh
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/common.sh"

verify_assert_home_is_ours
verify_export_isolation

fail=0

echo "verify-grog doctor"
echo "  RUN_ID:    ${VERIFY_RUN_ID}"
echo "  GROG_HOME: ${VERIFY_GROG_HOME}"
echo "  bin:       ${VERIFY_GROG_BIN}"

if [[ ! -x "${VERIFY_GROG_BIN}" ]]; then
  echo "FAIL binary missing or not executable: ${VERIFY_GROG_BIN}"
  echo "     run scripts/launch.sh first"
  fail=1
else
  ver="$("${VERIFY_GROG_BIN}" --version 2>&1 || true)"
  echo "  --version: ${ver//$'\n'/ / }"
  if [[ "${ver}" != grog\ * ]]; then
    echo "FAIL version must start with 'grog ', got: ${ver}"
    fail=1
  else
    echo "OK  identity is grog (not grok)"
  fi
fi

if [[ ! -d "${VERIFY_GROG_HOME}" ]]; then
  echo "FAIL GROG_HOME does not exist yet (${VERIFY_GROG_HOME}). launch.sh creates it."
  fail=1
elif [[ ! -f "${VERIFY_MARKER}" ]]; then
  echo "FAIL ${VERIFY_MARKER} missing — this home was not created by verify-grog. Do not drive it."
  fail=1
else
  echo "OK  GROG_HOME is owned by this run (${VERIFY_MARKER})"
fi

if [[ "${VERIFY_GROG_HOME}" == "${VERIFY_OFFICIAL_GROK_HOME}" ]]; then
  echo "FAIL GROG_HOME points at official grok ${VERIFY_OFFICIAL_GROK_HOME}"
  fail=1
fi

if [[ -e "${VERIFY_OFFICIAL_GROK_HOME}" ]]; then
  echo "WARN ${VERIFY_OFFICIAL_GROK_HOME} exists on this machine. Verification must not write it."
  echo "     snapshot: $(verify_official_grok_mtime)"
else
  echo "OK  official grok home ${VERIFY_OFFICIAL_GROK_HOME} is absent"
fi

if verify_tmux has-session -t "=${VERIFY_TMUX_SESSION}" 2>/dev/null; then
  echo "OK  tmux session ${VERIFY_TMUX_SESSION} is up (ours)"
else
  echo "INFO no tmux session ${VERIFY_TMUX_SESSION} — CLI drives do not need one"
fi

if [[ "${fail}" -ne 0 ]]; then
  echo "verify-grog doctor: NOT READY"
  exit 1
fi
echo "verify-grog doctor: READY"

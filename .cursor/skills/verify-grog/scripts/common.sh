#!/usr/bin/env bash
# Shared paths and isolation env for verify-grog.
# Source this; do not execute it.
set -euo pipefail

_VERIFY_SCRIPTS_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
VERIFY_SKILL_DIR="$(cd "${_VERIFY_SCRIPTS_DIR}/.." && pwd)"
VERIFY_REPO_ROOT="$(cd "${VERIFY_SKILL_DIR}/../../.." && pwd)"
VERIFY_ARTIFACTS="${VERIFY_ARTIFACTS:-${VERIFY_SKILL_DIR}/artifacts}"

VERIFY_RUN_ID="${VERIFY_RUN_ID:-${GROG_VERIFY_RUN_ID:-$(date +%Y%m%dT%H%M%S)-$$}}"
VERIFY_GROG_HOME="${VERIFY_GROG_HOME:-/tmp/grog-verify-${VERIFY_RUN_ID}}"
VERIFY_TMUX_SESSION="${VERIFY_TMUX_SESSION:-grog-verify-${VERIFY_RUN_ID}}"
VERIFY_TMUX_CONF="${VERIFY_TMUX_CONF:-/exec-daemon/tmux.portal.conf}"
VERIFY_MARKER="${VERIFY_GROG_HOME}/.verify-grog-owned"

# Public binary. Cargo also emits xai-grok-pager from the same main.rs.
VERIFY_GROG_BIN="${GROG_BIN:-${VERIFY_REPO_ROOT}/target/debug/grog}"

# Official grok and user grog homes — never write these from verification.
VERIFY_OFFICIAL_GROK_HOME="${HOME}/.grok"
VERIFY_USER_GROG_HOME="${HOME}/.grog"

verify_tmux() {
  if [[ -f "${VERIFY_TMUX_CONF}" ]]; then
    tmux -f "${VERIFY_TMUX_CONF}" "$@"
  else
    tmux "$@"
  fi
}

# Isolation envelope for every grog process this skill starts.
# GROG_HOME wins over GROK_HOME in xai-dirs. Unset GROK_HOME so a leftover
# official-grok override cannot redirect writes. Leader socket stays under
# our disposable home even if a drive forgets --no-leader.
verify_export_isolation() {
  export GROG_HOME="${VERIFY_GROG_HOME}"
  unset GROK_HOME || true
  export GROK_LEADER_SOCKET="${VERIFY_GROG_HOME}/leader.sock"
  export GROG_TELEMETRY_ENABLED="${GROG_TELEMETRY_ENABLED:-0}"
  export GROK_TELEMETRY_ENABLED="${GROK_TELEMETRY_ENABLED:-0}"
  export GROK_DISABLE_AUTOUPDATER="${GROK_DISABLE_AUTOUPDATER:-1}"
  export GROK_TELEMETRY_TRACE_UPLOAD="${GROK_TELEMETRY_TRACE_UPLOAD:-false}"
  export GROK_FEEDBACK_ENABLED="${GROK_FEEDBACK_ENABLED:-false}"
  export OTEL_SDK_DISABLED="${OTEL_SDK_DISABLED:-true}"
}

verify_assert_home_is_ours() {
  case "${VERIFY_GROG_HOME}" in
    /tmp/grog-verify-*) ;;
    *)
      echo "verify-grog: REFUSING GROG_HOME=${VERIFY_GROG_HOME}" >&2
      echo "verify-grog: must be /tmp/grog-verify-<run-id> (or set VERIFY_GROG_HOME to a disposable path you created)." >&2
      exit 2
      ;;
  esac
  if [[ "${VERIFY_GROG_HOME}" == "${VERIFY_OFFICIAL_GROK_HOME}" ]] ||
     [[ "${VERIFY_GROG_HOME}" == "${VERIFY_USER_GROG_HOME}" ]]; then
    echo "verify-grog: REFUSING to use official/user grok/grog home ${VERIFY_GROG_HOME}" >&2
    exit 2
  fi
}

verify_ensure_home() {
  verify_assert_home_is_ours
  mkdir -p "${VERIFY_GROG_HOME}"
  mkdir -p "${VERIFY_ARTIFACTS}"
  echo "${VERIFY_RUN_ID}" >"${VERIFY_MARKER}"
}

# Repo local builds need protoc. bin/protoc is a DotSlash wrapper.
# Prefer PROTOC if already set; else a working bin/protoc; else PATH; else a
# one-shot GitHub download of protoc 29.3 (same pin as bin/protoc).
verify_ensure_protoc() {
  if [[ -n "${PROTOC:-}" && -x "${PROTOC}" ]]; then
    return 0
  fi
  if [[ -x "${VERIFY_REPO_ROOT}/bin/protoc" ]] && "${VERIFY_REPO_ROOT}/bin/protoc" --version >/dev/null 2>&1; then
    return 0
  fi
  if command -v protoc >/dev/null 2>&1; then
    export PROTOC
    PROTOC="$(command -v protoc)"
    return 0
  fi
  local vendor="/tmp/grog-verify-protoc-29.3"
  if [[ ! -x "${vendor}/bin/protoc" ]]; then
    local zip="/tmp/grog-verify-protoc-29.3.zip"
    curl -fsSL -o "${zip}" \
      https://github.com/protocolbuffers/protobuf/releases/download/v29.3/protoc-29.3-linux-x86_64.zip
    mkdir -p "${vendor}"
    unzip -qo "${zip}" -d "${vendor}"
    chmod +x "${vendor}/bin/protoc"
  fi
  export PROTOC="${vendor}/bin/protoc"
}

verify_official_grok_mtime() {
  if [[ -e "${VERIFY_OFFICIAL_GROK_HOME}" ]]; then
    stat -c '%Y %n' "${VERIFY_OFFICIAL_GROK_HOME}" 2>/dev/null || stat -f '%m %N' "${VERIFY_OFFICIAL_GROK_HOME}"
  else
    echo "ABSENT ${VERIFY_OFFICIAL_GROK_HOME}"
  fi
}

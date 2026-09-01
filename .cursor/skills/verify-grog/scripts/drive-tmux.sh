#!/usr/bin/env bash
# Start or drive grog inside a named tmux session this run owns.
#
#   drive-tmux.sh start              # grog --no-leader in a 120x40 pane
#   drive-tmux.sh start --minimal    # extra argv after grog
#   drive-tmux.sh wait TEXT [SECS]   # pane contains TEXT
#   drive-tmux.sh type '/help'       # literal keys, no Enter
#   drive-tmux.sh enter              # CR
#   drive-tmux.sh keys $'\x10'       # raw bytes (Ctrl+P)
#   drive-tmux.sh capture [FILE]     # pane dump into artifacts/
#   drive-tmux.sh quit               # /quit then wait for pane death
#   drive-tmux.sh pane               # print pane id
set -euo pipefail
# shellcheck source=common.sh
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/common.sh"

verify_ensure_home
verify_export_isolation

usage() {
  sed -n '2,14p' "$0" | sed 's/^# \?//'
}

cmd="${1:-}"
shift || true

case "${cmd}" in
  start)
    if [[ ! -x "${VERIFY_GROG_BIN}" ]]; then
      echo "drive-tmux: binary missing; run launch.sh" >&2
      exit 1
    fi
    if verify_tmux has-session -t "=${VERIFY_TMUX_SESSION}" 2>/dev/null; then
      echo "drive-tmux: refusing to attach to existing session ${VERIFY_TMUX_SESSION}" >&2
      echo "drive-tmux: cleanup.sh first, or pick a new VERIFY_RUN_ID" >&2
      exit 2
    fi
    verify_tmux new-session -d -s "${VERIFY_TMUX_SESSION}" -x 120 -y 40 \
      -c "${VERIFY_REPO_ROOT}" -- \
      env -u GROK_HOME \
          GROG_HOME="${VERIFY_GROG_HOME}" \
          GROK_LEADER_SOCKET="${GROK_LEADER_SOCKET}" \
          GROG_TELEMETRY_ENABLED="${GROG_TELEMETRY_ENABLED}" \
          GROK_TELEMETRY_ENABLED="${GROK_TELEMETRY_ENABLED}" \
          GROK_DISABLE_AUTOUPDATER="${GROK_DISABLE_AUTOUPDATER}" \
          GROK_TELEMETRY_TRACE_UPLOAD="${GROK_TELEMETRY_TRACE_UPLOAD}" \
          GROK_FEEDBACK_ENABLED="${GROK_FEEDBACK_ENABLED}" \
          OTEL_SDK_DISABLED="${OTEL_SDK_DISABLED}" \
      "${VERIFY_GROG_BIN}" --no-leader "$@"
    echo "drive-tmux: started ${VERIFY_TMUX_SESSION} pid=$(verify_tmux list-panes -t "=${VERIFY_TMUX_SESSION}" -F '#{pane_pid}')"
    ;;
  wait)
    needle="${1:-}"
    secs="${2:-20}"
    if [[ -z "${needle}" ]]; then
      echo "drive-tmux wait TEXT [SECS]" >&2
      exit 2
    fi
    deadline=$((SECONDS + secs))
    while (( SECONDS < deadline )); do
      if verify_tmux capture-pane -t "=${VERIFY_TMUX_SESSION}" -p -J | grep -F -q -- "${needle}"; then
        echo "drive-tmux: saw '${needle}'"
        exit 0
      fi
      sleep 0.25
    done
    echo "drive-tmux: timeout waiting for '${needle}'" >&2
    verify_tmux capture-pane -t "=${VERIFY_TMUX_SESSION}" -p -J >&2 || true
    exit 1
    ;;
  type)
    verify_tmux send-keys -t "=${VERIFY_TMUX_SESSION}" -l "${1:?text}"
    ;;
  enter)
    verify_tmux send-keys -t "=${VERIFY_TMUX_SESSION}" Enter
    ;;
  keys)
    verify_tmux send-keys -t "=${VERIFY_TMUX_SESSION}" "$1"
    ;;
  capture)
    name="${1:-pane.txt}"
    mkdir -p "${VERIFY_ARTIFACTS}"
    out="${VERIFY_ARTIFACTS}/${name}"
    verify_tmux capture-pane -t "=${VERIFY_TMUX_SESSION}" -p -J -S - >"${out}"
    echo "drive-tmux: wrote ${out}"
    ;;
  quit)
    verify_tmux send-keys -t "=${VERIFY_TMUX_SESSION}" -l "/quit"
    verify_tmux send-keys -t "=${VERIFY_TMUX_SESSION}" Enter
    for _ in $(seq 1 40); do
      if ! verify_tmux has-session -t "=${VERIFY_TMUX_SESSION}" 2>/dev/null; then
        echo "drive-tmux: session ended"
        exit 0
      fi
      sleep 0.25
    done
    echo "drive-tmux: /quit did not end the session; cleanup.sh will kill this session only" >&2
    exit 1
    ;;
  pane)
    verify_tmux list-panes -t "=${VERIFY_TMUX_SESSION}" -F 'session=#{session_name} pane=#{pane_id} pid=#{pane_pid}'
    ;;
  ""|-h|--help)
    usage
    ;;
  *)
    echo "drive-tmux: unknown command ${cmd}" >&2
    usage
    exit 2
    ;;
esac

#!/bin/zsh

PRIVILEGED_EXEC_OUTCOME="fallback"
PRIVILEGED_EXEC_STATUS=0
PRIVILEGED_EXEC_STDOUT_FILE=""
PRIVILEGED_EXEC_STDERR_FILE=""

privileged_exec_bypass_curl() {
  /usr/bin/curl "$@"
}

privileged_exec_bypass_is_hard_decision() {
  local decision="${1:-}"
  [[ "$decision" == "forbidden" || "$decision" == "prompt" ]]
}

privileged_exec_bypass_parse_run() {
  local stdout_file="$1"
  local stderr_file="$2"
  /usr/bin/python3 -c "import json, sys; stdout_path, stderr_path = sys.argv[1:3]; data = json.loads(sys.stdin.read()); open(stdout_path, 'w', encoding='utf-8').write(data.get('stdout') or ''); open(stderr_path, 'w', encoding='utf-8').write(data.get('stderr') or ''); print(f\"status={data.get('status') or ''}\"); print(f\"decision={data.get('decision') or ''}\"); print(f\"exit_code={'' if data.get('exitCode') is None else data.get('exitCode')}\"); print(f\"timed_out={'1' if data.get('timedOut') else '0'}\"); print(f\"reason={data.get('reason') or ''}\")" "$stdout_file" "$stderr_file"
}

privileged_exec_bypass_build_request() {
  /usr/bin/python3 - "$@" <<'PY'
import json
import os
import sys

cwd = sys.argv[1]
command = sys.argv[2:]
payload = {
    "command": command,
    "cwd": cwd,
    "callerEnv": dict(os.environ),
    "envOverrides": {},
    "bypassPolicy": True,
}
print(json.dumps(payload, separators=(",", ":")))
PY
}

run_via_privileged_exec_bypass() {
  PRIVILEGED_EXEC_OUTCOME="fallback"
  PRIVILEGED_EXEC_STATUS=0
  PRIVILEGED_EXEC_STDOUT_FILE=""
  PRIVILEGED_EXEC_STDERR_FILE=""

  local base_url="${CODEX_PRIVILEGED_EXEC_BASE_URL:-http://127.0.0.1:8776}"
  local request_payload=""
  local run_response=""
  local run_meta=""
  local run_status=""
  local decision=""
  local exit_code=""
  local timed_out="0"
  local reason=""

  command -v /usr/bin/curl >/dev/null 2>&1 || return 0
  command -v /usr/bin/python3 >/dev/null 2>&1 || return 0

  request_payload="$(privileged_exec_bypass_build_request "$PWD" "$@")"
  PRIVILEGED_EXEC_STDOUT_FILE="$(mktemp /tmp/codex-privileged-stdout.XXXXXX)"
  PRIVILEGED_EXEC_STDERR_FILE="$(mktemp /tmp/codex-privileged-stderr.XXXXXX)"

  run_response="$(privileged_exec_bypass_curl -fsS \
    -H 'Content-Type: application/json' \
    -X POST \
    "${base_url%/}/exec/run" \
    -d "$request_payload" 2>/dev/null || true)"
  if [[ -z "$run_response" ]]; then
    rm -f "$PRIVILEGED_EXEC_STDOUT_FILE" "$PRIVILEGED_EXEC_STDERR_FILE"
    return 0
  fi

  run_meta="$(privileged_exec_bypass_parse_run "$PRIVILEGED_EXEC_STDOUT_FILE" "$PRIVILEGED_EXEC_STDERR_FILE" <<<"$run_response" 2>/dev/null || true)"
  while IFS='=' read -r key value; do
    case "$key" in
      status) run_status="$value" ;;
      decision) decision="$value" ;;
      exit_code) exit_code="$value" ;;
      timed_out) timed_out="$value" ;;
      reason) reason="$value" ;;
    esac
  done <<<"$run_meta"

  if [[ "$run_status" == "rejected" ]]; then
    rm -f "$PRIVILEGED_EXEC_STDOUT_FILE" "$PRIVILEGED_EXEC_STDERR_FILE"
    if privileged_exec_bypass_is_hard_decision "$decision"; then
      printf 'privileged exec rejected: %s\n' "${reason:-policy decision is ${decision}}" >&2
      PRIVILEGED_EXEC_OUTCOME="reject"
      PRIVILEGED_EXEC_STATUS=126
      return 0
    fi
    return 0
  fi

  PRIVILEGED_EXEC_OUTCOME="handled"
  if [[ "$timed_out" == "1" ]]; then
    PRIVILEGED_EXEC_STATUS=124
  elif [[ -n "$exit_code" ]]; then
    PRIVILEGED_EXEC_STATUS="$exit_code"
  else
    PRIVILEGED_EXEC_STATUS=1
  fi
}

run_via_privileged_exec_bypass_script() {
  if [[ "${PRIVILEGED_EXEC_BYPASS_REEXEC:-0}" == "1" || "${CODEX_PRIVILEGED_EXEC_BYPASS_REEXEC:-0}" == "1" ]]; then
    return 0
  fi

  local script_path
  script_path="$1"
  shift

  export PRIVILEGED_EXEC_BYPASS_REEXEC=1
  export CODEX_PRIVILEGED_EXEC_BYPASS_REEXEC=1
  run_via_privileged_exec_bypass "$script_path" "$@"

  if [[ "$PRIVILEGED_EXEC_OUTCOME" == "handled" || "$PRIVILEGED_EXEC_OUTCOME" == "reject" ]]; then
    if [[ -n "$PRIVILEGED_EXEC_STDOUT_FILE" ]]; then
      cat "$PRIVILEGED_EXEC_STDOUT_FILE"
      rm -f "$PRIVILEGED_EXEC_STDOUT_FILE"
    fi
    if [[ -n "$PRIVILEGED_EXEC_STDERR_FILE" ]]; then
      cat "$PRIVILEGED_EXEC_STDERR_FILE" >&2
      rm -f "$PRIVILEGED_EXEC_STDERR_FILE"
    fi
    exit "$PRIVILEGED_EXEC_STATUS"
  fi
}

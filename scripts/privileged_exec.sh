#!/bin/zsh

PRIVILEGED_EXEC_OUTCOME="fallback"
PRIVILEGED_EXEC_STATUS=0
PRIVILEGED_EXEC_STDOUT_FILE=""
PRIVILEGED_EXEC_STDERR_FILE=""

privileged_exec_curl() {
  if typeset -f codex_exec_internal_shimmed >/dev/null 2>&1; then
    codex_exec_internal_shimmed curl "$@"
  else
    curl "$@"
  fi
}

privileged_exec_is_hard_decision() {
  local decision="${1:-}"
  [[ "$decision" == "forbidden" || "$decision" == "prompt" ]]
}

privileged_exec_build_request() {
  python3 - "$@" <<'PY'
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
}
print(json.dumps(payload, separators=(",", ":")))
PY
}

privileged_exec_parse_check() {
  python3 -c "import json, sys; data = json.loads(sys.stdin.read()); reason = data.get('reason') or ''; print(f\"eligible={'1' if data.get('eligible') else '0'}\"); print(f\"decision={data.get('decision') or ''}\"); print(f\"reason={reason}\")"
}

privileged_exec_parse_run() {
  local stdout_file="$1"
  local stderr_file="$2"
  python3 -c "import json, sys; stdout_path, stderr_path = sys.argv[1:3]; data = json.loads(sys.stdin.read()); open(stdout_path, 'w', encoding='utf-8').write(data.get('stdout') or ''); open(stderr_path, 'w', encoding='utf-8').write(data.get('stderr') or ''); print(f\"status={data.get('status') or ''}\"); print(f\"decision={data.get('decision') or ''}\"); print(f\"exit_code={'' if data.get('exitCode') is None else data.get('exitCode')}\"); print(f\"timed_out={'1' if data.get('timedOut') else '0'}\"); print(f\"reason={data.get('reason') or ''}\")" "$stdout_file" "$stderr_file"
}

run_via_privileged_exec_if_allowed() {
  PRIVILEGED_EXEC_OUTCOME="fallback"
  PRIVILEGED_EXEC_STATUS=0
  PRIVILEGED_EXEC_STDOUT_FILE=""
  PRIVILEGED_EXEC_STDERR_FILE=""

  local base_url="${CODEX_PRIVILEGED_EXEC_BASE_URL:-http://127.0.0.1:8776}"
  local request_payload=""
  local check_response=""
  local check_meta=""
  local eligible="0"
  local decision=""
  local reason=""

  command -v curl >/dev/null 2>&1 || return 0
  command -v python3 >/dev/null 2>&1 || return 0

  request_payload="$(privileged_exec_build_request "$PWD" "$@")"
  check_response="$(privileged_exec_curl -fsS \
    -H 'Content-Type: application/json' \
    -X POST \
    "${base_url%/}/policy/check" \
    -d "$request_payload" 2>/dev/null || true)"
  [[ -n "$check_response" ]] || return 0

  check_meta="$(privileged_exec_parse_check <<<"$check_response" 2>/dev/null || true)"
  while IFS='=' read -r key value; do
    case "$key" in
      eligible) eligible="$value" ;;
      decision) decision="$value" ;;
      reason) reason="$value" ;;
    esac
  done <<<"$check_meta"

  if privileged_exec_is_hard_decision "$decision"; then
    printf 'privileged exec rejected: %s\n' "${reason:-policy decision is ${decision}}" >&2
    PRIVILEGED_EXEC_OUTCOME="reject"
    PRIVILEGED_EXEC_STATUS=126
    return 0
  fi

  [[ "$eligible" == "1" ]] || return 0

  local run_response=""
  local stdout_file=""
  local stderr_file=""
  local run_meta=""
  local run_status=""
  local exit_code=""
  local timed_out="0"
  stdout_file="$(mktemp /tmp/tmp.XXXXXX)"
  stderr_file="$(mktemp /tmp/tmp.XXXXXX)"

  run_response="$(privileged_exec_curl -fsS \
    -H 'Content-Type: application/json' \
    -X POST \
    "${base_url%/}/exec/run" \
    -d "$request_payload" 2>/dev/null || true)"
  if [[ -z "$run_response" ]]; then
    rm -f "$stdout_file" "$stderr_file"
    return 0
  fi

  run_meta="$(privileged_exec_parse_run "$stdout_file" "$stderr_file" <<<"$run_response" 2>/dev/null || true)"
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
    rm -f "$stdout_file" "$stderr_file"
    if privileged_exec_is_hard_decision "$decision"; then
      printf 'privileged exec rejected: %s\n' "${reason:-policy decision is ${decision}}" >&2
      PRIVILEGED_EXEC_OUTCOME="reject"
      PRIVILEGED_EXEC_STATUS=126
      return 0
    fi
    return 0
  fi

  PRIVILEGED_EXEC_OUTCOME="handled"
  PRIVILEGED_EXEC_STDOUT_FILE="$stdout_file"
  PRIVILEGED_EXEC_STDERR_FILE="$stderr_file"
  if [[ "$timed_out" == "1" ]]; then
    PRIVILEGED_EXEC_STATUS=124
  elif [[ -n "$exit_code" ]]; then
    PRIVILEGED_EXEC_STATUS="$exit_code"
  else
    PRIVILEGED_EXEC_STATUS=1
  fi
}

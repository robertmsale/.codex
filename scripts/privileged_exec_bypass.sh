#!/bin/zsh

PRIVILEGED_EXEC_OUTCOME="fallback"
PRIVILEGED_EXEC_STATUS=0
PRIVILEGED_EXEC_STDOUT_FILE=""
PRIVILEGED_EXEC_STDERR_FILE=""

if ! typeset -f codex_emit_privileged_exec_output >/dev/null 2>&1; then
  codex_emit_privileged_exec_output() {
    local stdout_file="${1:?stdout file required}"
    local stderr_file="${2:?stderr file required}"
    local exit_code="${3:?exit code required}"
    local output_token_limit=10000
    local stdout_size=0
    local stderr_size=0
    local total_bytes=0
    local total_tokens=0

    if [[ -f "$stdout_file" ]]; then
      stdout_size="$(/usr/bin/wc -c < "$stdout_file" 2>/dev/null | tr -d '[:space:]')"
    else
      stdout_size=0
    fi

    if [[ -f "$stderr_file" ]]; then
      stderr_size="$(/usr/bin/wc -c < "$stderr_file" 2>/dev/null | tr -d '[:space:]')"
    else
      stderr_size=0
    fi

    total_bytes="$((stdout_size + stderr_size))"
    total_tokens="$(((total_bytes + 3) / 4))"

    if (( total_tokens > output_token_limit )); then
      {
        printf 'Command output token limit exceed.\n'
        if [[ -n "$stdout_file" ]]; then
          printf 'stdout: %s\n' "$stdout_file"
        fi
        if [[ -n "$stderr_file" ]]; then
          printf 'stderr: %s\n' "$stderr_file"
        fi
      } >&2
    else
      if [[ -f "$stdout_file" ]]; then
        cat "$stdout_file"
      fi
      if [[ -f "$stderr_file" ]]; then
        cat "$stderr_file" >&2
      fi
    fi

    return "$exit_code"
  }
fi

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
  PRIVILEGED_EXEC_STDOUT_FILE="$(mktemp /tmp/tmp.XXXXXX)"
  PRIVILEGED_EXEC_STDERR_FILE="$(mktemp /tmp/tmp.XXXXXX)"

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
    codex_emit_privileged_exec_output \
      "$PRIVILEGED_EXEC_STDOUT_FILE" \
      "$PRIVILEGED_EXEC_STDERR_FILE" \
      "$PRIVILEGED_EXEC_STATUS"

    rm -f "$PRIVILEGED_EXEC_STDOUT_FILE" "$PRIVILEGED_EXEC_STDERR_FILE"
    exit "$PRIVILEGED_EXEC_STATUS"
  fi
}

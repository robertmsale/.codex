#!/bin/zsh

COMMAND_PARSER_AUTO_INITIALIZED="${COMMAND_PARSER_AUTO_INITIALIZED:-0}"
COMMAND_PARSER_AUTO_THRESHOLD_BYTES="${COMMAND_PARSER_AUTO_THRESHOLD_BYTES:-}"
COMMAND_PARSER_AUTO_LOG_DIR="${COMMAND_PARSER_AUTO_LOG_DIR:-}"
COMMAND_PARSER_AUTO_AUX_BASE_URL="${COMMAND_PARSER_AUTO_AUX_BASE_URL:-}"
COMMAND_PARSER_AUTO_PROFILE="${COMMAND_PARSER_AUTO_PROFILE:-}"
COMMAND_PARSER_AUTO_INCLUDE_WARNINGS="${COMMAND_PARSER_AUTO_INCLUDE_WARNINGS:-1}"

command_parser_auto_init() {
  [[ "${COMMAND_PARSER_AUTO_INITIALIZED:-0}" == "1" ]] && return 0

  local parser_script_dir=""
  parser_script_dir="$(cd "$HOME/.codex/scripts" && pwd)"

  if [[ -f "$parser_script_dir/command-parser.env" ]]; then
    set -a
    source "$parser_script_dir/command-parser.env"
    set +a
  fi

  COMMAND_PARSER_AUTO_THRESHOLD_BYTES="${COMMAND_PARSER_AUTO_THRESHOLD_BYTES:-32768}"
  COMMAND_PARSER_AUTO_LOG_DIR="${COMMAND_PARSER_AUTO_LOG_DIR:-${TMPDIR:-/tmp}/codex-command-parser}"
  COMMAND_PARSER_AUTO_AUX_BASE_URL="${CODEX_AUX_SERVER_BASE_URL:-http://127.0.0.1:8771}"
  COMMAND_PARSER_AUTO_PROFILE="${COMMAND_PARSER_PROFILE:-command-parser}"
  COMMAND_PARSER_AUTO_INCLUDE_WARNINGS="${COMMAND_PARSER_AUTO_INCLUDE_WARNINGS:-1}"
  mkdir -p "$COMMAND_PARSER_AUTO_LOG_DIR"
  COMMAND_PARSER_AUTO_INITIALIZED=1
}

command_parser_auto_combined_bytes() {
  local stdout_file="${1:?stdout file required}"
  local stderr_file="${2:?stderr file required}"
  local stdout_bytes="0"
  local stderr_bytes="0"

  if [[ -f "$stdout_file" ]]; then
    stdout_bytes="$(wc -c <"$stdout_file" | tr -d '[:space:]')"
  fi
  if [[ -f "$stderr_file" ]]; then
    stderr_bytes="$(wc -c <"$stderr_file" | tr -d '[:space:]')"
  fi
  printf '%s\n' "$((stdout_bytes + stderr_bytes))"
}

command_parser_auto_write_log() {
  local log_file="${1:?log file required}"
  local stdout_file="${2:?stdout file required}"
  local stderr_file="${3:?stderr file required}"
  local exit_code="${4:?exit code required}"
  shift 4
  local command=("$@")

  python3 - "$log_file" "$stdout_file" "$stderr_file" "$exit_code" "${command[@]}" <<'PY'
import pathlib
import shlex
import sys

log_path = pathlib.Path(sys.argv[1])
stdout_path = pathlib.Path(sys.argv[2])
stderr_path = pathlib.Path(sys.argv[3])
exit_code = sys.argv[4]
command = sys.argv[5:]

stdout = stdout_path.read_text(encoding="utf-8", errors="replace") if stdout_path.exists() else ""
stderr = stderr_path.read_text(encoding="utf-8", errors="replace") if stderr_path.exists() else ""

parts = [
    "# command\n",
    shlex.join(command) + "\n",
    "\n# exit_code\n",
    f"{exit_code}\n",
    "\n# stdout\n",
    stdout,
]
if stdout and not stdout.endswith("\n"):
    parts.append("\n")
parts.extend([
    "\n# stderr\n",
    stderr,
])
if stderr and not stderr.endswith("\n"):
    parts.append("\n")

log_path.write_text("".join(parts), encoding="utf-8")
PY
}

command_parser_auto_request_payload() {
  local combined_log_file="${1:?combined log file required}"
  shift
  python3 - "$combined_log_file" "$COMMAND_PARSER_AUTO_INCLUDE_WARNINGS" "$COMMAND_PARSER_AUTO_PROFILE" "$@" <<'PY'
import json
import pathlib
import sys

log_path = pathlib.Path(sys.argv[1])
include_warnings = sys.argv[2] not in {"0", "", "false", "False"}
profile = sys.argv[3]
command = sys.argv[4:]

payload = {
    "command": command,
    "output": log_path.read_text(encoding="utf-8", errors="replace"),
    "includeWarnings": include_warnings,
    "profile": profile,
}
print(json.dumps(payload, separators=(",", ":")))
PY
}

command_parser_auto_parse() {
  local combined_log_file="${1:?combined log file required}"
  shift
  local payload=""

  payload="$(command_parser_auto_request_payload "$combined_log_file" "$@")"
  curl -fsS \
    -H 'Content-Type: application/json' \
    -X POST \
    "${COMMAND_PARSER_AUTO_AUX_BASE_URL%/}/v1/command-parser/parse" \
    -d "$payload"
}

command_parser_auto_render_response() {
  python3 -c '
import json
import sys

data = json.loads(sys.stdin.read())
if not data.get("ok"):
    message = (data.get("error") or "command-parser server request failed").strip()
    print(message, file=sys.stderr)
    raise SystemExit(1)
print((data.get("message") or "").rstrip())
'
}

command_parser_auto_should_skip() {
  local command=("$@")
  local token=""
  local base=""

  [[ "${IS_USING_COMMAND_PARSER:-}" == "true" ]] && return 0
  [[ "${CODEX_COMMAND_PARSER_ACTIVE:-}" == "1" ]] && return 0

  case "${PWD:-}" in
    */codex-aux/command-parser-*|*/codex-command-parser/*)
      return 0
      ;;
  esac

  [[ ${#command[@]} -gt 0 ]] || return 1
  base="$(basename "${command[0]}")"

  case "$base" in
    command-parser)
      return 0
      ;;
  esac

  for token in "${command[@]}"; do
    case "$token" in
      *output.log*|*.log|*/codex-command-parser/*|*/codex-aux/*)
        return 0
        ;;
    esac
  done

  case "$base" in
    cat|tail|head|less|more)
      return 0
      ;;
    sed)
      for token in "${command[@]:1}"; do
        case "$token" in
          *.log|*output.log*)
            return 0
            ;;
        esac
      done
      ;;
    rg|grep)
      for token in "${command[@]:1}"; do
        case "$token" in
          *.log|*output.log*)
            return 0
            ;;
        esac
      done
      ;;
    python3|python)
      for token in "${command[@]:1}"; do
        case "$token" in
          *output.log*|*.log)
            return 0
            ;;
        esac
      done
      ;;
  esac

  return 1
}

command_postprocess_output() {
  local stdout_file="${1:?stdout file required}"
  local stderr_file="${2:?stderr file required}"
  local exit_code="${3:?exit code required}"
  shift 3
  local command=("$@")

  command_parser_auto_init

  local combined_bytes="0"
  combined_bytes="$(command_parser_auto_combined_bytes "$stdout_file" "$stderr_file")"

  if [[ "$combined_bytes" -le "$COMMAND_PARSER_AUTO_THRESHOLD_BYTES" ]] || command_parser_auto_should_skip "${command[@]}"; then
    [[ -f "$stdout_file" ]] && cat "$stdout_file"
    [[ -f "$stderr_file" ]] && cat "$stderr_file" >&2
    return "$exit_code"
  fi

  local log_file=""
  local final_log_file=""
  local parser_response=""
  local parser_message=""
  log_file="$(mktemp "$COMMAND_PARSER_AUTO_LOG_DIR/command-output.XXXXXX")"
  final_log_file="${log_file}.log"
  mv "$log_file" "$final_log_file"
  log_file="$final_log_file"
  command_parser_auto_write_log "$log_file" "$stdout_file" "$stderr_file" "$exit_code" "${command[@]}"

  if parser_response="$(command_parser_auto_parse "$log_file" "${command[@]}" 2>/dev/null)" \
    && parser_message="$(command_parser_auto_render_response <<<"$parser_response" 2>/dev/null)"; then
    [[ -n "$parser_message" ]] && printf '%s\n' "$parser_message"
    printf 'full log: %s\n' "$log_file" >&2
    return "$exit_code"
  fi

  [[ -f "$stdout_file" ]] && cat "$stdout_file"
  [[ -f "$stderr_file" ]] && cat "$stderr_file" >&2
  printf 'full log: %s\n' "$log_file" >&2
  return "$exit_code"
}

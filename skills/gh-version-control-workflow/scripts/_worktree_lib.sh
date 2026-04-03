#!/bin/bash
set -euo pipefail

gitops_http_base_url() {
  printf '%s\n' "${PARALLELS_SYNC_GITOPS_BASE_URL:-http://127.0.0.1:8765}"
}

bridge_fallback_allowed() {
  [[ "${PARALLELS_SYNC_GITOPS_NO_BRIDGE:-0}" != "1" ]]
}

normalize_mirrored_path() {
  local raw_path="$1"
  local host_user="${HOME##*/}"
  local normalized="$raw_path"
  case "$raw_path" in
    "/home/$host_user" | "/home/$host_user/"*)
      normalized="/Users/$host_user${raw_path#"/home/$host_user"}"
      ;;
  esac
  if [[ ! -e "$normalized" && "$normalized" == "/Users/$host_user/Code/"* ]]; then
    local without_code="/Users/$host_user/${normalized#"/Users/$host_user/Code/"}"
    if [[ -e "$without_code" ]]; then
      normalized="$without_code"
    fi
  fi
  printf '%s\n' "$normalized"
}

remove_shadow_worktree_path() {
  local worktree_path="$1"
  local attempt
  worktree_path="$(normalize_mirrored_path "$worktree_path")"
  [[ -n "$worktree_path" ]] || return 0
  [[ -e "$worktree_path" ]] || return 0

  # macOS can transiently report "Directory not empty" immediately after writes.
  # Shadow cleanup is best-effort and must not fail the higher-level workflow.
  for attempt in 1 2 3; do
    rm -rf "$worktree_path" 2>/dev/null || true
    [[ ! -e "$worktree_path" ]] && return 0
    sleep 0.2
  done

  if [[ -e "$worktree_path" ]]; then
    echo "WARNING: shadow worktree path still exists after cleanup attempt: $worktree_path" >&2
  fi
}

json_escape() {
  python3 - <<'PY' "$1"
import json
import sys
print(json.dumps(sys.argv[1]))
PY
}

json_build_request() {
  python3 - <<'PY' "$@"
import json
import sys

payload = {"args": {}}
for item in sys.argv[1:]:
    key, value = item.split("=", 1)
    if value.startswith("json:"):
        payload["args"][key] = json.loads(value[5:])
    elif value in {"true", "false"}:
        payload["args"][key] = value == "true"
    else:
        payload["args"][key] = value
print(json.dumps(payload))
PY
}

http_gitops_op() {
  local op="$1"
  shift
  local payload
  payload="$(json_build_request "$@")"
  local url
  url="$(gitops_http_base_url)/v1/ops/$op"
  local response_body
  local http_code
  response_body="$(mktemp)"
  http_code="$(
    curl -sS \
      -o "$response_body" \
      -w '%{http_code}' \
      -H 'Content-Type: application/json' \
      -d "$payload" \
      "$url"
  )" || {
    local curl_status=$?
    rm -f "$response_body"
    echo "gitops HTTP request failed for $op" >&2
    return "$curl_status"
  }
  if [[ "$http_code" -lt 200 || "$http_code" -ge 300 ]]; then
    python3 - <<'PY' "$response_body" "$op" "$http_code" >&2
import json
import pathlib
import sys

body_path = pathlib.Path(sys.argv[1])
op = sys.argv[2]
http_code = sys.argv[3]
raw = body_path.read_text(encoding="utf-8", errors="replace").strip()
message = ""
if raw:
    try:
        payload = json.loads(raw)
    except json.JSONDecodeError:
        message = raw
    else:
        detail = payload.get("detail")
        if isinstance(detail, str) and detail.strip():
            message = detail.strip()
        else:
            message = raw
if not message:
    message = f"gitops HTTP request failed for {op} with HTTP {http_code}"
print(message)
PY
    local parse_status=$?
    rm -f "$response_body"
    return 1
  fi
  local response
  response="$(cat "$response_body")"
  rm -f "$response_body"
  python3 - <<'PY' "$response"
import json
import sys

data = json.loads(sys.argv[1])
result = data.get("result", "")
if isinstance(result, str):
    print(result)
else:
    print(json.dumps(result))
PY
}

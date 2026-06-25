#!/usr/bin/env bash
set -euo pipefail

if [[ "${ROBDEX_AGENT_RUNTIME_LIVE_SERVER_VALIDATION:-}" != "1" ]]; then
  echo "Refusing live model validation. Set ROBDEX_AGENT_RUNTIME_LIVE_SERVER_VALIDATION=1." >&2
  exit 2
fi

BASE_URL="${ROBDEX_AGENT_RUNTIME_SERVER_BASE_URL:-http://127.0.0.1:8765}"
DATABASE_URL="${ROBDEX_AGENT_RUNTIME_DATABASE_URL:-postgres://postgres@127.0.0.1:5432/robdex_agent_runtime}"
ROLE_ID="${ROBDEX_AGENT_RUNTIME_LIVE_SERVER_ROLE:-runtime-live-server-gpt54mini-validation}"
WORKDIR="${ROBDEX_AGENT_RUNTIME_LIVE_SERVER_WORKDIR:-$(pwd)}"
PROJECT="${ROBDEX_AGENT_RUNTIME_LIVE_SERVER_PROJECT:-agent-runtime-live-validation}"

tmpdir="$(mktemp -d)"
cleanup() {
  rm -rf "$tmpdir"
}
trap cleanup EXIT

role_file="$tmpdir/$ROLE_ID.json"

python3 - "$ROLE_ID" > "$role_file" <<'PY'
import json
import sys
from pathlib import Path

role_id = sys.argv[1]
seed = json.loads(Path("roles/runtime-no-rg.json").read_text())
seed["id"] = role_id
seed["version"] = "1.0.54"
seed["displayName"] = "Runtime Live Server Validation GPT 5.4 Mini"
seed["modelDefaults"]["model"] = "gpt-5.4-mini"
seed["modelDefaults"]["reasoningEffort"] = "low"
seed["instructionText"] = (
    "You are a live server validation role. For the user's request, call only "
    "execute_code with harmless read-only Starlark. Prefer fs.read(\"Cargo.toml\") "
    "or print a small constant. Do not request mutations."
)
json.dump(seed, sys.stdout, indent=2)
PY

echo "Importing live validation role $ROLE_ID with model gpt-5.4-mini"
cargo run --quiet --bin robdex-agent-runtime -- roles import "$role_file" >/dev/null

echo "Checking server health at $BASE_URL/health"
curl -fsS "$BASE_URL/health" >/dev/null

session_json="$(curl -fsS -X POST "$BASE_URL/sessions" \
  -H 'content-type: application/json' \
  -d "{\"role\":\"$ROLE_ID\",\"project\":\"$PROJECT\",\"workdir\":\"$WORKDIR\",\"title\":\"live server validation gpt-5.4-mini\",\"name\":\"live-server-validation\"}")"
session_id="$(python3 -c 'import json,sys; print(json.load(sys.stdin)["sessionId"])' <<<"$session_json")"
echo "session_id=$session_id"

send_json="$(curl -fsS -X POST "$BASE_URL/sessions/$session_id/send" \
  -H 'content-type: application/json' \
  -d '{"message":"Use execute_code with exactly this harmless read-only Starlark: content = fs.read(\"Cargo.toml\"); print({\"validation\":\"ok\",\"contains_workspace\":\"workspace\" in content})"}')"
turn_id="$(python3 -c 'import json,sys; print(json.load(sys.stdin)["turnId"])' <<<"$send_json")"
echo "turn_id=$turn_id"
echo "send_result=$send_json"

echo "DB evidence:"
psql "$DATABASE_URL" -X -v ON_ERROR_STOP=1 -P pager=off -c "
SELECT 'turn' AS evidence, id::text, status FROM turns WHERE id = '$turn_id';
SELECT 'model_event' AS evidence, id::text, event_type, payload->>'model' AS model, payload->>'tool' AS tool
FROM model_events
WHERE turn_id = '$turn_id'
ORDER BY created_at;
SELECT 'tool_call' AS evidence, id::text, tool_name, status
FROM tool_calls
WHERE turn_id = '$turn_id'
ORDER BY created_at;
SELECT 'script_run' AS evidence, sr.id::text, sr.status
FROM script_runs sr
JOIN tool_calls tc ON tc.id = sr.tool_call_id
WHERE tc.turn_id = '$turn_id'
ORDER BY sr.created_at;
SELECT 'event_stream' AS evidence, sequence, event_type, status
FROM event_stream
WHERE turn_id = '$turn_id'
ORDER BY sequence;
SELECT 'mutation_event_count' AS evidence, count(*)::text AS count
FROM event_stream
WHERE turn_id = '$turn_id' AND event_type LIKE 'mutation.%';
"

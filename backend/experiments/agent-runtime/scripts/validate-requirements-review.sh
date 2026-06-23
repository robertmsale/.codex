#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
source scripts/lib-validation-db.sh

SERVER_PID=""
cleanup_server() {
  if [[ -n "${SERVER_PID:-}" ]] && kill -0 "$SERVER_PID" 2>/dev/null; then
    kill "$SERVER_PID" 2>/dev/null || true
    wait "$SERVER_PID" 2>/dev/null || true
  fi
}
cleanup_all() {
  local status=$?
  cleanup_server || true
  validation_cleanup_database
  return "$status"
}
validation_setup_database
trap cleanup_all EXIT

export ROBDEX_AGENT_RUNTIME_EMBEDDING_PROVIDER=disabled
export ROBDEX_AGENT_RUNTIME_SERVER_HOST="${ROBDEX_AGENT_RUNTIME_SERVER_VALIDATION_HOST:-127.0.0.1}"
PORT="${ROBDEX_AGENT_RUNTIME_SERVER_VALIDATION_PORT:-}"
if [[ -z "$PORT" ]]; then
  PORT="$(python3 - <<'PY'
import socket
s=socket.socket()
s.bind(("127.0.0.1",0))
print(s.getsockname()[1])
s.close()
PY
)"
fi
export ROBDEX_AGENT_RUNTIME_SERVER_PORT="$PORT"
BASE_URL="http://${ROBDEX_AGENT_RUNTIME_SERVER_HOST}:${ROBDEX_AGENT_RUNTIME_SERVER_PORT}"

run() {
  printf '\n$ %s\n' "$*"
  "$@"
}

printf '[requirements-validation] database=%s\n' "$ROBDEX_AGENT_RUNTIME_DATABASE_URL"
printf '[requirements-validation] base_url=%s\n' "$BASE_URL"

run cargo run --quiet --bin robdex-agent-runtime -- init-db
run cargo run --quiet --bin robdex-agent-runtime -- roles import-seeds
run cargo build --quiet --bin robdex-agent-runtime-server

SERVER_LOG="/tmp/robdex-agent-runtime-requirements-validation-${PORT}.log"
rm -f "$SERVER_LOG"
target/debug/robdex-agent-runtime-server --host "$ROBDEX_AGENT_RUNTIME_SERVER_HOST" --port "$ROBDEX_AGENT_RUNTIME_SERVER_PORT" >"$SERVER_LOG" 2>&1 &
SERVER_PID="$!"
for _ in {1..200}; do
  if curl -fsS "$BASE_URL/health" >/tmp/requirements-review-health.json 2>/tmp/requirements-review-health.err; then
    break
  fi
  if ! kill -0 "$SERVER_PID" 2>/dev/null; then
    cat "$SERVER_LOG" >&2 || true
    exit 1
  fi
  sleep 0.1
done
curl -fsS "$BASE_URL/health" >/tmp/requirements-review-health.json

SESSION_ID="$(cargo run --quiet --bin robdex-agent-runtime -- sessions new --role runtime-no-rg --project requirements-smoke --workdir . --worktree-root .)"
REQ_FILE="/tmp/requirements-review-smoke-set-${PORT}.json"
CLAIM_FILE="/tmp/requirements-review-smoke-claim-${PORT}.json"
VERDICT_FILE="/tmp/requirements-review-smoke-verdict-${PORT}.json"
cat >"$REQ_FILE" <<'JSON'
{"title":"requirements smoke","requirements":[{"key":"smoke_passes","statement":"Resident smoke validates Requirements Review.","severity":"must","verificationMethod":{"method":"smoke"}}]}
JSON
curl -fsS -X POST "$BASE_URL/sessions/$SESSION_ID/requirements" -H 'content-type: application/json' --data @"$REQ_FILE" >/tmp/requirements-review-set.json
TURN_ID="$(python3 - "$ROBDEX_AGENT_RUNTIME_DATABASE_URL" "$SESSION_ID" <<'PY'
import subprocess, sys, uuid
db, session = sys.argv[1:]
turn = str(uuid.uuid4())
sql = f"INSERT INTO turns (id, session_id, role, input_text, status, started_at) VALUES ('{turn}','{session}','user','smoke claim','completed',now())"
subprocess.check_call(["psql", db, "-v", "ON_ERROR_STOP=1", "-Atc", sql], stdout=subprocess.DEVNULL)
print(turn)
PY
)"
cat >"$CLAIM_FILE" <<'JSON'
{"summary":"smoke done","requirements":{"smoke_passes":{"claim":"satisfied","evidence":["resident smoke"],"justification":"reviewable claim","risk":"low"}}}
JSON
cargo run --quiet --bin robdex-agent-runtime -- requirements record-claim --session "$SESSION_ID" --turn "$TURN_ID" "$CLAIM_FILE" >/tmp/requirements-review-claim.json
REVIEWER_ID="$(psql "$ROBDEX_AGENT_RUNTIME_DATABASE_URL" -v ON_ERROR_STOP=1 -Atc "select reviewer_session_id from requirement_review_bindings where source_session_id='$SESSION_ID'")"
REVIEWER_TURN_ID="$(python3 - "$ROBDEX_AGENT_RUNTIME_DATABASE_URL" "$REVIEWER_ID" <<'PY'
import subprocess, sys, uuid
db, session = sys.argv[1:]
turn = str(uuid.uuid4())
sql = f"INSERT INTO turns (id, session_id, role, input_text, status, started_at) VALUES ('{turn}','{session}','user','smoke verdict','completed',now())"
subprocess.check_call(["psql", db, "-v", "ON_ERROR_STOP=1", "-Atc", sql], stdout=subprocess.DEVNULL)
print(turn)
PY
)"
cat >"$VERDICT_FILE" <<'JSON'
{"summary":"smoke passes","requirements":{"smoke_passes":{"verdict":"pass","evidence":["resident smoke"],"justification":"accepted","risk":"low"}},"overallVerdict":"pass","route":"source"}
JSON
cargo run --quiet --bin robdex-agent-runtime -- requirements record-verdict --reviewer "$REVIEWER_ID" --turn "$REVIEWER_TURN_ID" "$VERDICT_FILE" >/tmp/requirements-review-verdict.json
curl -fsS "$BASE_URL/sessions/$SESSION_ID/requirements/packets" >/tmp/requirements-review-packets.json
curl -fsS "$BASE_URL/sessions" >/tmp/requirements-review-sessions.json
python3 - "$SESSION_ID" "$REVIEWER_ID" <<'PY'
import json, sys
source, reviewer = sys.argv[1:]
packets=json.load(open('/tmp/requirements-review-packets.json'))['packets']
sessions_doc=json.load(open('/tmp/requirements-review-sessions.json'))
sessions=sessions_doc['sessions'] if isinstance(sessions_doc, dict) else sessions_doc
assert any(p['packetKind']=='claim' for p in packets), packets
assert any(p['packetKind']=='verdict' for p in packets), packets
ids={s['id'] for s in sessions}
assert source in ids, ids
assert reviewer not in ids, ids
PY
printf '[requirements-validation] source=%s reviewer=%s packets and session filtering ok\n' "$SESSION_ID" "$REVIEWER_ID"

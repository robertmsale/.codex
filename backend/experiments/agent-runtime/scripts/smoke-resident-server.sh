#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
source scripts/lib-validation-db.sh

SERVER_PID=""
HOLD_WS_PID=""
cleanup_server() {
  if [[ -n "${HOLD_WS_PID:-}" ]] && kill -0 "$HOLD_WS_PID" 2>/dev/null; then
    printf '[server-smoke] stopping hold websocket client pid=%s\n' "$HOLD_WS_PID"
    kill "$HOLD_WS_PID" 2>/dev/null || true
    wait "$HOLD_WS_PID" 2>/dev/null || true
  fi
  if [[ -n "${SERVER_PID:-}" ]] && kill -0 "$SERVER_PID" 2>/dev/null; then
    printf '[server-smoke] stopping server pid=%s\n' "$SERVER_PID"
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
export ROBDEX_AGENT_RUNTIME_SERVER_HOST="${ROBDEX_AGENT_RUNTIME_SERVER_SMOKE_HOST:-127.0.0.1}"
PORT="${ROBDEX_AGENT_RUNTIME_SERVER_SMOKE_PORT:-}"
if [[ -z "$PORT" ]]; then
  PORT="$(python3 - <<'PY'
import socket
s = socket.socket()
s.bind(("127.0.0.1", 0))
print(s.getsockname()[1])
s.close()
PY
)"
fi
export ROBDEX_AGENT_RUNTIME_SERVER_PORT="$PORT"
BASE_URL="http://${ROBDEX_AGENT_RUNTIME_SERVER_HOST}:${ROBDEX_AGENT_RUNTIME_SERVER_PORT}"
export ROBDEX_AGENT_RUNTIME_SERVER_SMOKE_BASE_URL="$BASE_URL"

run() {
  printf '\n$ %s\n' "$*"
  "$@"
}

printf '[server-smoke] database=%s\n' "$ROBDEX_AGENT_RUNTIME_DATABASE_URL"
printf '[server-smoke] base_url=%s\n' "$BASE_URL"

run cargo run --quiet --bin robdex-agent-runtime -- init-db
run cargo run --quiet --bin robdex-agent-runtime -- roles import-seeds
ROLE_VERSION_COUNT_BEFORE_SERVER="$(psql "$ROBDEX_AGENT_RUNTIME_DATABASE_URL" -v ON_ERROR_STOP=1 -Atc "select count(*) from role_versions")"
ROLE_IMPORTED_EVENTS_BEFORE_SERVER="$(psql "$ROBDEX_AGENT_RUNTIME_DATABASE_URL" -v ON_ERROR_STOP=1 -Atc "select count(*) from event_stream where event_type='role.imported'")"
PRESEEDED_SESSION_ID="$(cargo run --quiet --bin robdex-agent-runtime -- sessions new --role runtime-no-rg --project server-smoke-restart --workdir . --worktree-root .)"
PRESEEDED_PROCESS_ID="$(python3 - "$ROBDEX_AGENT_RUNTIME_DATABASE_URL" "$PRESEEDED_SESSION_ID" <<'PY'
import subprocess, sys, uuid
db, session = sys.argv[1:]
process_id = str(uuid.uuid4())
sql = """
INSERT INTO managed_processes (
  id, handle, session_id, starting_turn_id, command_version_id, binary_name, argv, cwd,
  os_pid, os_pgid, status, end_of_turn_behavior, end_of_session_behavior, max_runtime_ms, metadata
) VALUES (
  '{process_id}', 'preseeded-restart-process', '{session}', NULL, NULL, 'sleep', '["sleep","60"]'::jsonb, '.',
  NULL, NULL, 'running', 'continue', 'terminate', NULL, '{{"source":"smoke-resident-server"}}'::jsonb
)
""".format(process_id=process_id, session=session)
subprocess.check_call(["psql", db, "-v", "ON_ERROR_STOP=1", "-Atc", sql], stdout=subprocess.DEVNULL)
print(process_id)
PY
)"
export ROBDEX_AGENT_RUNTIME_SERVER_SMOKE_PRESEEDED_PROCESS_ID="$PRESEEDED_PROCESS_ID"
export ROBDEX_AGENT_RUNTIME_SERVER_SMOKE_PRESEEDED_SESSION_ID="$PRESEEDED_SESSION_ID"
printf '[server-smoke] preseeded running process session=%s process=%s\n' "$PRESEEDED_SESSION_ID" "$PRESEEDED_PROCESS_ID"
run cargo build --quiet --bin robdex-agent-runtime-server --bin robdex-agent-runtime-server-smoke-client

SERVER_BIN="${ROBDEX_AGENT_RUNTIME_SERVER_BIN:-target/debug/robdex-agent-runtime-server}"
SERVER_LOG="${ROBDEX_AGENT_RUNTIME_SERVER_SMOKE_LOG:-/tmp/robdex-agent-runtime-server-smoke-${ROBDEX_AGENT_RUNTIME_SERVER_PORT}.log}"
rm -f "$SERVER_LOG"
printf '[server-smoke] starting server: %s --host %s --port %s\n' "$SERVER_BIN" "$ROBDEX_AGENT_RUNTIME_SERVER_HOST" "$ROBDEX_AGENT_RUNTIME_SERVER_PORT"
"$SERVER_BIN" --host "$ROBDEX_AGENT_RUNTIME_SERVER_HOST" --port "$ROBDEX_AGENT_RUNTIME_SERVER_PORT" >"$SERVER_LOG" 2>&1 &
SERVER_PID="$!"

HEALTH_DEADLINE_SECONDS="${ROBDEX_AGENT_RUNTIME_SERVER_SMOKE_HEALTH_DEADLINE_SECONDS:-20}"
HEALTH_OK=0
for ((attempt=1; attempt<=HEALTH_DEADLINE_SECONDS*10; attempt++)); do
  if ! kill -0 "$SERVER_PID" 2>/dev/null; then
    printf '[server-smoke] server exited before health check passed. log follows:\n' >&2
    cat "$SERVER_LOG" >&2 || true
    exit 1
  fi
  if curl -fsS "$BASE_URL/health" >/tmp/robdex-agent-runtime-server-smoke-health.json 2>/tmp/robdex-agent-runtime-server-smoke-health.err; then
    HEALTH_OK=1
    break
  fi
  sleep 0.1
done

if [[ "$HEALTH_OK" != "1" ]]; then
  printf '[server-smoke] server did not become healthy within %ss\n' "$HEALTH_DEADLINE_SECONDS" >&2
  printf '[server-smoke] last curl error:\n' >&2
  cat /tmp/robdex-agent-runtime-server-smoke-health.err >&2 || true
  printf '[server-smoke] server log:\n' >&2
  cat "$SERVER_LOG" >&2 || true
  exit 1
fi

printf '[server-smoke] health ok: %s\n' "$(cat /tmp/robdex-agent-runtime-server-smoke-health.json)"
run target/debug/robdex-agent-runtime-server-smoke-client
HOLD_WS_READY="/tmp/robdex-agent-runtime-server-smoke-hold-ws-${ROBDEX_AGENT_RUNTIME_SERVER_PORT}.ready"
HOLD_WS_LOG="/tmp/robdex-agent-runtime-server-smoke-hold-ws-${ROBDEX_AGENT_RUNTIME_SERVER_PORT}.log"
rm -f "$HOLD_WS_READY" "$HOLD_WS_LOG"
printf '[server-smoke] starting hold websocket client\n'
ROBDEX_AGENT_RUNTIME_SERVER_SMOKE_HOLD_WS=1 \
ROBDEX_AGENT_RUNTIME_SERVER_SMOKE_HOLD_WS_READY="$HOLD_WS_READY" \
target/debug/robdex-agent-runtime-server-smoke-client >"$HOLD_WS_LOG" 2>&1 &
HOLD_WS_PID="$!"
HOLD_READY=0
for ((attempt=1; attempt<=100; attempt++)); do
  if ! kill -0 "$HOLD_WS_PID" 2>/dev/null; then
    printf '[server-smoke] hold websocket client exited before ready. log follows:\n' >&2
    cat "$HOLD_WS_LOG" >&2 || true
    exit 1
  fi
  if [[ -f "$HOLD_WS_READY" ]]; then
    HOLD_READY=1
    break
  fi
  sleep 0.1
done
if [[ "$HOLD_READY" != "1" ]]; then
  printf '[server-smoke] hold websocket client did not become ready. log follows:\n' >&2
  cat "$HOLD_WS_LOG" >&2 || true
  exit 1
fi
printf '[server-smoke] requesting graceful shutdown pid=%s\n' "$SERVER_PID"
kill "$SERVER_PID"
wait "$SERVER_PID"
SERVER_PID=""
wait "$HOLD_WS_PID"
HOLD_WS_PID=""
if ! rg -q 'websocket observed serverShutdown' "$HOLD_WS_LOG"; then
  printf '[server-smoke] hold websocket did not observe server shutdown. log follows:\n' >&2
  cat "$HOLD_WS_LOG" >&2 || true
  exit 1
fi
if ! rg -q '"lostProcesses":1' "$SERVER_LOG"; then
  printf '[server-smoke] startup report did not contain expected reconciliation count. log follows:\n' >&2
  cat "$SERVER_LOG" >&2 || true
  exit 1
fi
ROLE_VERSION_COUNT_AFTER_SERVER="$(psql "$ROBDEX_AGENT_RUNTIME_DATABASE_URL" -v ON_ERROR_STOP=1 -Atc "select count(*) from role_versions")"
ROLE_IMPORTED_EVENTS_AFTER_SERVER="$(psql "$ROBDEX_AGENT_RUNTIME_DATABASE_URL" -v ON_ERROR_STOP=1 -Atc "select count(*) from event_stream where event_type='role.imported'")"
if [[ "$ROLE_VERSION_COUNT_AFTER_SERVER" != "$ROLE_VERSION_COUNT_BEFORE_SERVER" ]]; then
  printf '[server-smoke] seed role idempotence failed: role_versions before=%s after=%s\n' "$ROLE_VERSION_COUNT_BEFORE_SERVER" "$ROLE_VERSION_COUNT_AFTER_SERVER" >&2
  exit 1
fi
if [[ "$ROLE_IMPORTED_EVENTS_AFTER_SERVER" != "$ROLE_IMPORTED_EVENTS_BEFORE_SERVER" ]]; then
  printf '[server-smoke] seed role idempotence failed: role.imported events before=%s after=%s\n' "$ROLE_IMPORTED_EVENTS_BEFORE_SERVER" "$ROLE_IMPORTED_EVENTS_AFTER_SERVER" >&2
  exit 1
fi
if ! rg -q '"seedRolesImported":0' "$SERVER_LOG" || ! rg -q '"seedRolesUnchanged":3' "$SERVER_LOG"; then
  printf '[server-smoke] startup report missing seed role idempotence counts. log follows:\n' >&2
  cat "$SERVER_LOG" >&2 || true
  exit 1
fi
if ! rg -q '\[server-shutdown\]' "$SERVER_LOG"; then
  printf '[server-smoke] shutdown report missing. log follows:\n' >&2
  cat "$SERVER_LOG" >&2 || true
  exit 1
fi
SECOND_SERVER_LOG="${SERVER_LOG}.second"
rm -f "$SECOND_SERVER_LOG"
printf '[server-smoke] starting second server to prove seed-role startup idempotence\n'
"$SERVER_BIN" --host "$ROBDEX_AGENT_RUNTIME_SERVER_HOST" --port "$ROBDEX_AGENT_RUNTIME_SERVER_PORT" >"$SECOND_SERVER_LOG" 2>&1 &
SERVER_PID="$!"
SECOND_HEALTH_OK=0
for ((attempt=1; attempt<=HEALTH_DEADLINE_SECONDS*10; attempt++)); do
  if ! kill -0 "$SERVER_PID" 2>/dev/null; then
    printf '[server-smoke] second server exited before health check passed. log follows:\n' >&2
    cat "$SECOND_SERVER_LOG" >&2 || true
    exit 1
  fi
  if curl -fsS "$BASE_URL/health" >/tmp/robdex-agent-runtime-server-smoke-health-second.json 2>/tmp/robdex-agent-runtime-server-smoke-health-second.err; then
    SECOND_HEALTH_OK=1
    break
  fi
  sleep 0.1
done
if [[ "$SECOND_HEALTH_OK" != "1" ]]; then
  printf '[server-smoke] second server did not become healthy within %ss\n' "$HEALTH_DEADLINE_SECONDS" >&2
  cat /tmp/robdex-agent-runtime-server-smoke-health-second.err >&2 || true
  cat "$SECOND_SERVER_LOG" >&2 || true
  exit 1
fi
kill "$SERVER_PID"
wait "$SERVER_PID"
SERVER_PID=""
ROLE_VERSION_COUNT_AFTER_SECOND="$(psql "$ROBDEX_AGENT_RUNTIME_DATABASE_URL" -v ON_ERROR_STOP=1 -Atc "select count(*) from role_versions")"
ROLE_IMPORTED_EVENTS_AFTER_SECOND="$(psql "$ROBDEX_AGENT_RUNTIME_DATABASE_URL" -v ON_ERROR_STOP=1 -Atc "select count(*) from event_stream where event_type='role.imported'")"
if [[ "$ROLE_VERSION_COUNT_AFTER_SECOND" != "$ROLE_VERSION_COUNT_BEFORE_SERVER" ]]; then
  printf '[server-smoke] second startup seed role idempotence failed: role_versions before=%s after_second=%s\n' "$ROLE_VERSION_COUNT_BEFORE_SERVER" "$ROLE_VERSION_COUNT_AFTER_SECOND" >&2
  exit 1
fi
if [[ "$ROLE_IMPORTED_EVENTS_AFTER_SECOND" != "$ROLE_IMPORTED_EVENTS_BEFORE_SERVER" ]]; then
  printf '[server-smoke] second startup seed role idempotence failed: role.imported events before=%s after_second=%s\n' "$ROLE_IMPORTED_EVENTS_BEFORE_SERVER" "$ROLE_IMPORTED_EVENTS_AFTER_SECOND" >&2
  exit 1
fi
if ! rg -q '"seedRolesImported":0' "$SECOND_SERVER_LOG" || ! rg -q '"seedRolesUnchanged":3' "$SECOND_SERVER_LOG"; then
  printf '[server-smoke] second startup report missing seed role idempotence counts. log follows:\n' >&2
  cat "$SECOND_SERVER_LOG" >&2 || true
  exit 1
fi
printf '\n[server-smoke] deterministic resident server smoke complete\n'

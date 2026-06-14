#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
source scripts/lib-validation-db.sh

SERVICE_STATE_DIR=""
cleanup_service_validation() {
  local status=$?
  if [[ -n "${SERVICE_STATE_DIR:-}" ]]; then
    ROBDEX_AGENT_RUNTIME_SERVICE_STATE_DIR="$SERVICE_STATE_DIR" scripts/agent-runtime-service.sh stop --force >/tmp/robdex-agent-runtime-service-cleanup.log 2>&1 || true
    rm -rf "$SERVICE_STATE_DIR"
  fi
  validation_cleanup_database
  return "$status"
}

validation_setup_database
trap cleanup_service_validation EXIT

export ROBDEX_AGENT_RUNTIME_EMBEDDING_PROVIDER=disabled
export ROBDEX_AGENT_RUNTIME_SERVER_HOST="${ROBDEX_AGENT_RUNTIME_SERVICE_VALIDATION_HOST:-127.0.0.1}"
if [[ -z "${ROBDEX_AGENT_RUNTIME_SERVER_PORT:-}" ]]; then
  ROBDEX_AGENT_RUNTIME_SERVER_PORT="$(python3 - <<'PY'
import socket
s = socket.socket()
s.bind(("127.0.0.1", 0))
print(s.getsockname()[1])
s.close()
PY
)"
  export ROBDEX_AGENT_RUNTIME_SERVER_PORT
fi
SERVICE_STATE_DIR="${ROBDEX_AGENT_RUNTIME_SERVICE_VALIDATION_STATE_DIR:-$(mktemp -d /tmp/robdex-agent-runtime-service.XXXXXX)}"
export ROBDEX_AGENT_RUNTIME_SERVICE_STATE_DIR="$SERVICE_STATE_DIR"
export ROBDEX_AGENT_RUNTIME_SERVICE_HEALTH_DEADLINE_SECONDS="${ROBDEX_AGENT_RUNTIME_SERVICE_HEALTH_DEADLINE_SECONDS:-20}"
BASE_URL="http://${ROBDEX_AGENT_RUNTIME_SERVER_HOST}:${ROBDEX_AGENT_RUNTIME_SERVER_PORT}"
SERVICE="scripts/agent-runtime-service.sh"

printf '[service-validation] database=%s\n' "$ROBDEX_AGENT_RUNTIME_DATABASE_URL"
printf '[service-validation] state_dir=%s\n' "$SERVICE_STATE_DIR"
printf '[service-validation] base_url=%s\n' "$BASE_URL"

printf '[service-validation] start\n'
START_OUTPUT="$($SERVICE start)"
printf '%s\n' "$START_OUTPUT"
PID1="$(cat "$SERVICE_STATE_DIR/server.pid")"
if [[ -z "$PID1" ]] || ! kill -0 "$PID1" 2>/dev/null; then
  printf '[service-validation] started pid is not alive: %s\n' "$PID1" >&2
  exit 1
fi
if ! curl -fsS "$BASE_URL/health" >"$SERVICE_STATE_DIR/validation-health.json"; then
  printf '[service-validation] health check failed after start\n' >&2
  $SERVICE status >&2 || true
  exit 1
fi
if ! rg -q '\[server-startup\]' "$SERVICE_STATE_DIR/server.stdout.log"; then
  printf '[service-validation] startup log evidence missing\n' >&2
  $SERVICE logs >&2 || true
  exit 1
fi
if [[ ! -s "$SERVICE_STATE_DIR/effective-config.json" ]]; then
  printf '[service-validation] effective config snapshot missing\n' >&2
  exit 1
fi

printf '[service-validation] status\n'
STATUS_OUTPUT="$($SERVICE status)"
printf '%s\n' "$STATUS_OUTPUT"
if ! printf '%s\n' "$STATUS_OUTPUT" | rg -q '^state=running$'; then
  printf '[service-validation] status did not report running\n' >&2
  exit 1
fi
if ! printf '%s\n' "$STATUS_OUTPUT" | rg -q '^health=ok:'; then
  printf '[service-validation] status did not report health ok\n' >&2
  exit 1
fi

printf '[service-validation] duplicate start must fail\n'
if $SERVICE start >"$SERVICE_STATE_DIR/duplicate-start.out" 2>"$SERVICE_STATE_DIR/duplicate-start.err"; then
  printf '[service-validation] duplicate start unexpectedly succeeded\n' >&2
  exit 1
fi
if ! rg -q 'refusing duplicate start' "$SERVICE_STATE_DIR/duplicate-start.err"; then
  printf '[service-validation] duplicate start diagnostic missing\n' >&2
  cat "$SERVICE_STATE_DIR/duplicate-start.err" >&2 || true
  exit 1
fi

printf '[service-validation] logs\n'
LOG_OUTPUT="$($SERVICE logs)"
if ! printf '%s\n' "$LOG_OUTPUT" | rg -q '\[server-startup\]'; then
  printf '[service-validation] logs command did not include startup report\n' >&2
  exit 1
fi

printf '[service-validation] restart\n'
$SERVICE restart
PID2="$(cat "$SERVICE_STATE_DIR/server.pid")"
if [[ -z "$PID2" ]] || ! kill -0 "$PID2" 2>/dev/null; then
  printf '[service-validation] restarted pid is not alive: %s\n' "$PID2" >&2
  exit 1
fi
if [[ "$PID1" == "$PID2" ]]; then
  printf '[service-validation] restart did not produce a new pid: %s\n' "$PID2" >&2
  exit 1
fi
if ! curl -fsS "$BASE_URL/health" >"$SERVICE_STATE_DIR/validation-health-after-restart.json"; then
  printf '[service-validation] health check failed after restart\n' >&2
  $SERVICE status >&2 || true
  exit 1
fi
if kill -0 "$PID1" 2>/dev/null; then
  printf '[service-validation] old pid still alive after restart: %s\n' "$PID1" >&2
  exit 1
fi

printf '[service-validation] stop\n'
$SERVICE stop
if [[ -f "$SERVICE_STATE_DIR/server.pid" ]]; then
  printf '[service-validation] pid file still exists after stop\n' >&2
  exit 1
fi
if kill -0 "$PID2" 2>/dev/null; then
  printf '[service-validation] pid still alive after stop: %s\n' "$PID2" >&2
  exit 1
fi
STOPPED_STATUS="$($SERVICE status)"
printf '%s\n' "$STOPPED_STATUS"
if ! printf '%s\n' "$STOPPED_STATUS" | rg -q '^state=stopped$'; then
  printf '[service-validation] status did not report stopped after stop\n' >&2
  exit 1
fi

printf '[service-validation] deterministic local service validation complete\n'

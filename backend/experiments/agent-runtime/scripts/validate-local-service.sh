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

json_get() {
  local path="$1"
  local expr="$2"
  python3 - "$path" "$expr" <<'PY'
import json, sys
with open(sys.argv[1], encoding="utf-8") as fh:
    value = json.load(fh)
for part in sys.argv[2].split("."):
    if part:
        value = value[part]
if isinstance(value, bool):
    print("true" if value else "false")
elif value is None:
    print("")
else:
    print(value)
PY
}

assert_json_eq() {
  local path="$1"
  local expr="$2"
  local expected="$3"
  local actual
  actual="$(json_get "$path" "$expr")"
  if [[ "$actual" != "$expected" ]]; then
    printf '[service-validation] expected %s=%s in %s, got %s\n' "$expr" "$expected" "$path" "$actual" >&2
    exit 1
  fi
}

assert_no_secret_leak() {
  local path="$1"
  if rg -q 'postgres:postgres@|postgresql://[^[:space:]]+:[^*][^@]*@|token|secret' "$path"; then
    printf '[service-validation] discovery/config output appears to contain an unredacted secret: %s\n' "$path" >&2
    cat "$path" >&2
    exit 1
  fi
}

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
if [[ ! -s "$SERVICE_STATE_DIR/discovery.json" ]]; then
  printf '[service-validation] discovery packet missing after start\n' >&2
  exit 1
fi
assert_json_eq "$SERVICE_STATE_DIR/discovery.json" "serviceState" "running"
assert_json_eq "$SERVICE_STATE_DIR/discovery.json" "stateFlags.running" "true"
assert_json_eq "$SERVICE_STATE_DIR/discovery.json" "healthResult.ok" "true"
assert_json_eq "$SERVICE_STATE_DIR/discovery.json" "baseUrl" "$BASE_URL"
assert_json_eq "$SERVICE_STATE_DIR/discovery.json" "healthUrl" "$BASE_URL/health"
assert_json_eq "$SERVICE_STATE_DIR/discovery.json" "webSocketUrl" "ws://${ROBDEX_AGENT_RUNTIME_SERVER_HOST}:${ROBDEX_AGENT_RUNTIME_SERVER_PORT}/state/ws"
assert_no_secret_leak "$SERVICE_STATE_DIR/discovery.json"

printf '[service-validation] discover\n'
DISCOVER_OUTPUT_FILE="$SERVICE_STATE_DIR/discover-output.json"
$SERVICE discover >"$DISCOVER_OUTPUT_FILE"
assert_json_eq "$DISCOVER_OUTPUT_FILE" "serviceState" "running"
assert_json_eq "$DISCOVER_OUTPUT_FILE" "pidLiveness.alive" "true"
assert_json_eq "$DISCOVER_OUTPUT_FILE" "paths.discoveryFile" "$SERVICE_STATE_DIR/discovery.json"
assert_no_secret_leak "$DISCOVER_OUTPUT_FILE"
if ! cmp -s "$DISCOVER_OUTPUT_FILE" "$SERVICE_STATE_DIR/discovery.json"; then
  printf '[service-validation] discover output differs from persisted discovery file\n' >&2
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
assert_json_eq "$SERVICE_STATE_DIR/discovery.json" "serviceState" "running"

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
assert_json_eq "$SERVICE_STATE_DIR/discovery.json" "serviceState" "running"
assert_json_eq "$SERVICE_STATE_DIR/discovery.json" "pid" "$PID2"
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
assert_json_eq "$SERVICE_STATE_DIR/discovery.json" "serviceState" "stopped"
assert_json_eq "$SERVICE_STATE_DIR/discovery.json" "stateFlags.stopped" "true"

printf '[service-validation] stale pid diagnostics\n'
printf '999999\n' >"$SERVICE_STATE_DIR/server.pid"
if $SERVICE status >"$SERVICE_STATE_DIR/stale-status.out" 2>"$SERVICE_STATE_DIR/stale-status.err"; then
  printf '[service-validation] stale pid status unexpectedly succeeded\n' >&2
  exit 1
fi
assert_json_eq "$SERVICE_STATE_DIR/discovery.json" "serviceState" "stalePid"
assert_json_eq "$SERVICE_STATE_DIR/discovery.json" "stateFlags.stalePid" "true"
rm -f "$SERVICE_STATE_DIR/server.pid"
$SERVICE discover >"$SERVICE_STATE_DIR/stopped-discover.json"

printf '[service-validation] unhealthy diagnostics\n'
printf '%s\n' "$$" >"$SERVICE_STATE_DIR/server.pid"
$SERVICE discover >"$SERVICE_STATE_DIR/unhealthy-discover.json"
assert_json_eq "$SERVICE_STATE_DIR/unhealthy-discover.json" "serviceState" "unhealthy"
assert_json_eq "$SERVICE_STATE_DIR/unhealthy-discover.json" "stateFlags.unhealthy" "true"
rm -f "$SERVICE_STATE_DIR/server.pid"

printf '[service-validation] missing config diagnostics\n'
mv "$SERVICE_STATE_DIR/effective-config.json" "$SERVICE_STATE_DIR/effective-config.json.saved"
$SERVICE discover >"$SERVICE_STATE_DIR/missing-config-discover.json"
assert_json_eq "$SERVICE_STATE_DIR/missing-config-discover.json" "serviceState" "missingConfig"
assert_json_eq "$SERVICE_STATE_DIR/missing-config-discover.json" "stateFlags.missingConfig" "true"
mv "$SERVICE_STATE_DIR/effective-config.json.saved" "$SERVICE_STATE_DIR/effective-config.json"

printf '[service-validation] deterministic local service validation complete\n'

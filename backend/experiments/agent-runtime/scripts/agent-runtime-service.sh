#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

DEFAULT_DATABASE_URL="postgres://postgres:postgres@127.0.0.1:5432/robdex_agent_runtime"
STATE_DIR="${ROBDEX_AGENT_RUNTIME_SERVICE_STATE_DIR:-.runtime-service}"
PID_FILE="$STATE_DIR/server.pid"
STDOUT_LOG="$STATE_DIR/server.stdout.log"
STDERR_LOG="$STATE_DIR/server.stderr.log"
CONFIG_FILE="$STATE_DIR/effective-config.json"
HOST="${ROBDEX_AGENT_RUNTIME_SERVER_HOST:-127.0.0.1}"
PORT="${ROBDEX_AGENT_RUNTIME_SERVER_PORT:-8765}"
DATABASE_URL="${ROBDEX_AGENT_RUNTIME_DATABASE_URL:-$DEFAULT_DATABASE_URL}"
BASE_URL="http://${HOST}:${PORT}"
HEALTH_DEADLINE_SECONDS="${ROBDEX_AGENT_RUNTIME_SERVICE_HEALTH_DEADLINE_SECONDS:-20}"
STOP_DEADLINE_SECONDS="${ROBDEX_AGENT_RUNTIME_SERVICE_STOP_DEADLINE_SECONDS:-15}"

redact_database_url() {
  python3 - "$DATABASE_URL" <<'PY'
import sys
from urllib.parse import urlsplit, urlunsplit
url = sys.argv[1]
parts = urlsplit(url)
netloc = parts.netloc
if '@' in netloc:
    userinfo, host = netloc.rsplit('@', 1)
    if ':' in userinfo:
        user = userinfo.split(':', 1)[0]
        netloc = f"{user}:***@{host}"
    else:
        netloc = f"{userinfo}@{host}"
print(urlunsplit((parts.scheme, netloc, parts.path, parts.query, parts.fragment)))
PY
}

is_running_pid() {
  local pid="$1"
  [[ -n "$pid" ]] && kill -0 "$pid" 2>/dev/null
}

pid_from_file() {
  if [[ -f "$PID_FILE" ]]; then
    tr -d '[:space:]' < "$PID_FILE"
  fi
}

ensure_state_dir() {
  mkdir -p "$STATE_DIR"
}

build_or_locate_server() {
  if [[ -n "${ROBDEX_AGENT_RUNTIME_SERVER_BIN:-}" ]]; then
    if [[ ! -x "$ROBDEX_AGENT_RUNTIME_SERVER_BIN" ]]; then
      printf '[agent-runtime-service] configured server binary is not executable: %s\n' "$ROBDEX_AGENT_RUNTIME_SERVER_BIN" >&2
      return 1
    fi
    printf '%s\n' "$ROBDEX_AGENT_RUNTIME_SERVER_BIN"
    return 0
  fi
  if [[ ! -x target/debug/robdex-agent-runtime-server ]]; then
    cargo build --quiet --bin robdex-agent-runtime-server
  fi
  printf '%s\n' "target/debug/robdex-agent-runtime-server"
}

write_config_snapshot() {
  local server_bin="$1"
  local pid="${2:-}"
  local redacted_db
  redacted_db="$(redact_database_url)"
  python3 - "$CONFIG_FILE" <<PY
import json, os, sys
path = sys.argv[1]
config = {
    "baseUrl": "$BASE_URL",
    "bindHost": "$HOST",
    "bindPort": int("$PORT"),
    "databaseUrlRedacted": "$redacted_db",
    "serverBinary": "$server_bin",
    "pid": "$pid" or None,
    "pidFile": "$PID_FILE",
    "stdoutLog": "$STDOUT_LOG",
    "stderrLog": "$STDERR_LOG",
    "schemaPolicy": os.environ.get("ROBDEX_AGENT_RUNTIME_SCHEMA_POLICY", "apply"),
    "seedRolePolicy": os.environ.get("ROBDEX_AGENT_RUNTIME_SEED_ROLE_POLICY", "importSeeds"),
    "commandBootstrapPolicy": os.environ.get("ROBDEX_AGENT_RUNTIME_COMMAND_BOOTSTRAP_POLICY", "bootstrapDefaults"),
    "processReconciliationPolicy": os.environ.get("ROBDEX_AGENT_RUNTIME_PROCESS_RECONCILIATION_POLICY", "markRunningLost"),
    "shutdownPolicy": os.environ.get("ROBDEX_AGENT_RUNTIME_SHUTDOWN_POLICY", "gracefulMarkRunningLost"),
}
with open(path, "w", encoding="utf-8") as fh:
    json.dump(config, fh, indent=2, sort_keys=True)
    fh.write("\n")
PY
}

wait_for_health() {
  local pid="$1"
  local health_file="$STATE_DIR/health.json"
  local health_err="$STATE_DIR/health.err"
  local attempts=$((HEALTH_DEADLINE_SECONDS * 10))
  for ((attempt=1; attempt<=attempts; attempt++)); do
    if ! is_running_pid "$pid"; then
      printf '[agent-runtime-service] server exited before health passed. stderr log: %s\n' "$STDERR_LOG" >&2
      [[ -f "$STDERR_LOG" ]] && cat "$STDERR_LOG" >&2 || true
      return 1
    fi
    if curl -fsS "$BASE_URL/health" >"$health_file" 2>"$health_err"; then
      return 0
    fi
    sleep 0.1
  done
  printf '[agent-runtime-service] server did not become healthy within %ss\n' "$HEALTH_DEADLINE_SECONDS" >&2
  printf '[agent-runtime-service] last curl error:\n' >&2
  [[ -f "$health_err" ]] && cat "$health_err" >&2 || true
  printf '[agent-runtime-service] stdout log: %s\n' "$STDOUT_LOG" >&2
  [[ -f "$STDOUT_LOG" ]] && cat "$STDOUT_LOG" >&2 || true
  printf '[agent-runtime-service] stderr log: %s\n' "$STDERR_LOG" >&2
  [[ -f "$STDERR_LOG" ]] && cat "$STDERR_LOG" >&2 || true
  return 1
}

start_service() {
  ensure_state_dir
  local existing_pid
  existing_pid="$(pid_from_file || true)"
  if [[ -n "$existing_pid" ]] && is_running_pid "$existing_pid"; then
    printf '[agent-runtime-service] refusing duplicate start; server already running pid=%s base_url=%s\n' "$existing_pid" "$BASE_URL" >&2
    printf '[agent-runtime-service] use: scripts/agent-runtime-service.sh restart\n' >&2
    return 1
  fi
  if [[ -n "$existing_pid" ]]; then
    printf '[agent-runtime-service] stale pid file detected: %s pid=%s\n' "$PID_FILE" "$existing_pid"
    mv "$PID_FILE" "$PID_FILE.stale.$(date +%s)"
  fi
  local server_bin
  server_bin="$(build_or_locate_server)"
  : >"$STDOUT_LOG"
  : >"$STDERR_LOG"
  write_config_snapshot "$server_bin" ""
  ROBDEX_AGENT_RUNTIME_DATABASE_URL="$DATABASE_URL" \
  ROBDEX_AGENT_RUNTIME_SERVER_HOST="$HOST" \
  ROBDEX_AGENT_RUNTIME_SERVER_PORT="$PORT" \
    "$server_bin" --host "$HOST" --port "$PORT" >"$STDOUT_LOG" 2>"$STDERR_LOG" &
  local pid="$!"
  printf '%s\n' "$pid" >"$PID_FILE"
  write_config_snapshot "$server_bin" "$pid"
  if ! wait_for_health "$pid"; then
    return 1
  fi
  printf '[agent-runtime-service] started\n'
  printf 'base_url=%s\n' "$BASE_URL"
  printf 'pid=%s\n' "$pid"
  printf 'pid_file=%s\n' "$PID_FILE"
  printf 'stdout_log=%s\n' "$STDOUT_LOG"
  printf 'stderr_log=%s\n' "$STDERR_LOG"
  printf 'config=%s\n' "$CONFIG_FILE"
  printf 'database=%s\n' "$(redact_database_url)"
}

stop_service() {
  local force="0"
  if [[ "${1:-}" == "--force" ]]; then
    force="1"
  fi
  local pid
  pid="$(pid_from_file || true)"
  if [[ -z "$pid" ]]; then
    printf '[agent-runtime-service] stopped: no pid file at %s\n' "$PID_FILE"
    return 0
  fi
  if ! is_running_pid "$pid"; then
    printf '[agent-runtime-service] stale pid file removed: %s pid=%s\n' "$PID_FILE" "$pid"
    rm -f "$PID_FILE"
    return 0
  fi
  printf '[agent-runtime-service] stopping pid=%s\n' "$pid"
  kill "$pid" 2>/dev/null || true
  for ((attempt=1; attempt<=STOP_DEADLINE_SECONDS*10; attempt++)); do
    if ! is_running_pid "$pid"; then
      rm -f "$PID_FILE"
      printf '[agent-runtime-service] stopped pid=%s\n' "$pid"
      return 0
    fi
    sleep 0.1
  done
  if [[ "$force" == "1" ]]; then
    printf '[agent-runtime-service] graceful stop timed out; force killing pid=%s\n' "$pid" >&2
    kill -KILL "$pid" 2>/dev/null || true
    for ((attempt=1; attempt<=50; attempt++)); do
      if ! is_running_pid "$pid"; then
        rm -f "$PID_FILE"
        printf '[agent-runtime-service] force stopped pid=%s\n' "$pid"
        return 0
      fi
      sleep 0.1
    done
    printf '[agent-runtime-service] force kill did not stop pid=%s\n' "$pid" >&2
    return 1
  fi
  printf '[agent-runtime-service] graceful stop timed out for pid=%s; rerun stop --force to kill\n' "$pid" >&2
  return 1
}

status_service() {
  local pid state health redacted_db
  pid="$(pid_from_file || true)"
  redacted_db="$(redact_database_url)"
  if [[ -z "$pid" ]]; then
    state="stopped"
  elif is_running_pid "$pid"; then
    state="running"
  else
    state="stale"
  fi
  health="not_checked"
  if [[ "$state" == "running" ]]; then
    if curl -fsS "$BASE_URL/health" >"$STATE_DIR/status-health.json" 2>"$STATE_DIR/status-health.err"; then
      health="ok: $(cat "$STATE_DIR/status-health.json")"
    else
      health="failed: $(cat "$STATE_DIR/status-health.err" 2>/dev/null || true)"
    fi
  fi
  printf 'state=%s\n' "$state"
  printf 'pid=%s\n' "${pid:-}"
  printf 'pid_file=%s\n' "$PID_FILE"
  printf 'base_url=%s\n' "$BASE_URL"
  printf 'health=%s\n' "$health"
  printf 'database=%s\n' "$redacted_db"
  printf 'stdout_log=%s\n' "$STDOUT_LOG"
  printf 'stderr_log=%s\n' "$STDERR_LOG"
  printf 'config=%s\n' "$CONFIG_FILE"
  if [[ "$state" == "stale" ]]; then
    return 2
  fi
}

restart_service() {
  stop_service "${1:-}"
  start_service
}

logs_service() {
  local mode="${1:-print}"
  case "$mode" in
    --tail|-f|tail)
      printf '[agent-runtime-service] tailing stdout=%s stderr=%s\n' "$STDOUT_LOG" "$STDERR_LOG"
      tail -n "${ROBDEX_AGENT_RUNTIME_SERVICE_LOG_LINES:-80}" -f "$STDOUT_LOG" "$STDERR_LOG"
      ;;
    print|"")
      printf '==> %s <==\n' "$STDOUT_LOG"
      [[ -f "$STDOUT_LOG" ]] && cat "$STDOUT_LOG" || true
      printf '==> %s <==\n' "$STDERR_LOG"
      [[ -f "$STDERR_LOG" ]] && cat "$STDERR_LOG" || true
      ;;
    *)
      printf 'usage: %s logs [--tail]\n' "$0" >&2
      return 2
      ;;
  esac
}

usage() {
  cat <<USAGE
usage: $0 <start|stop|status|restart|logs> [--force|--tail]

Environment:
  ROBDEX_AGENT_RUNTIME_SERVICE_STATE_DIR   state directory, default .runtime-service
  ROBDEX_AGENT_RUNTIME_DATABASE_URL        runtime Postgres URL
  ROBDEX_AGENT_RUNTIME_SERVER_HOST         bind host, default 127.0.0.1
  ROBDEX_AGENT_RUNTIME_SERVER_PORT         bind port, default 8765
  ROBDEX_AGENT_RUNTIME_SERVER_BIN          optional existing server binary
USAGE
}

command="${1:-}"
case "$command" in
  start) start_service ;;
  stop) shift; stop_service "${1:-}" ;;
  status) status_service ;;
  restart) shift; restart_service "${1:-}" ;;
  logs) shift; logs_service "${1:-print}" ;;
  -h|--help|help|"") usage ;;
  *) usage >&2; exit 2 ;;
esac

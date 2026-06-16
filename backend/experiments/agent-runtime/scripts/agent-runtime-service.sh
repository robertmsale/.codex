#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

DEFAULT_DATABASE_URL="postgres://postgres:postgres@127.0.0.1:5432/robdex_agent_runtime"
default_state_dir() {
  if [[ "$(uname -s)" == "Darwin" ]]; then
    printf '%s\n' "$HOME/Library/Application Support/Robdex Agent Runtime/service"
  else
    printf '%s\n' "${XDG_STATE_HOME:-$HOME/.local/state}/robdex-agent-runtime/service"
  fi
}

STATE_DIR="${ROBDEX_AGENT_RUNTIME_SERVICE_STATE_DIR:-$(default_state_dir)}"
PID_FILE="$STATE_DIR/server.pid"
STDOUT_LOG="$STATE_DIR/server.stdout.log"
STDERR_LOG="$STATE_DIR/server.stderr.log"
CONFIG_FILE="$STATE_DIR/effective-config.json"
DISCOVERY_FILE="$STATE_DIR/discovery.json"
PACKAGE_FILE="$STATE_DIR/service-package.json"
LAUNCHD_LABEL="com.robdex.agent-runtime.experimental"
LAUNCHD_PLIST="$HOME/Library/LaunchAgents/$LAUNCHD_LABEL.plist"
HOST="${ROBDEX_AGENT_RUNTIME_SERVER_HOST:-127.0.0.1}"
PORT="${ROBDEX_AGENT_RUNTIME_SERVER_PORT:-8765}"
DATABASE_URL="${ROBDEX_AGENT_RUNTIME_DATABASE_URL:-$DEFAULT_DATABASE_URL}"
BASE_URL="http://${HOST}:${PORT}"
HEALTH_URL="$BASE_URL/health"
WEBSOCKET_URL="ws://${HOST}:${PORT}/state/ws"
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
    "discoveryFile": "$DISCOVERY_FILE",
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

discovery_packet() {
  local write_mode="${1:-print}"
  ensure_state_dir
  ROBDEX_AGENT_RUNTIME_DISCOVERY_WRITE="$write_mode" \
  ROBDEX_AGENT_RUNTIME_DISCOVERY_FILE="$DISCOVERY_FILE" \
  ROBDEX_AGENT_RUNTIME_DISCOVERY_CONFIG_FILE="$CONFIG_FILE" \
  ROBDEX_AGENT_RUNTIME_DISCOVERY_PID_FILE="$PID_FILE" \
  ROBDEX_AGENT_RUNTIME_DISCOVERY_STDOUT_LOG="$STDOUT_LOG" \
  ROBDEX_AGENT_RUNTIME_DISCOVERY_STDERR_LOG="$STDERR_LOG" \
  ROBDEX_AGENT_RUNTIME_DISCOVERY_STATE_DIR="$STATE_DIR" \
  ROBDEX_AGENT_RUNTIME_DISCOVERY_BASE_URL="$BASE_URL" \
  ROBDEX_AGENT_RUNTIME_DISCOVERY_HEALTH_URL="$HEALTH_URL" \
  ROBDEX_AGENT_RUNTIME_DISCOVERY_WEBSOCKET_URL="$WEBSOCKET_URL" \
  ROBDEX_AGENT_RUNTIME_DISCOVERY_DATABASE_URL="$DATABASE_URL" \
  ROBDEX_AGENT_RUNTIME_DISCOVERY_HOST="$HOST" \
  ROBDEX_AGENT_RUNTIME_DISCOVERY_PORT="$PORT" \
  ROBDEX_AGENT_RUNTIME_DISCOVERY_SCHEMA_POLICY="${ROBDEX_AGENT_RUNTIME_SCHEMA_POLICY:-apply}" \
  ROBDEX_AGENT_RUNTIME_DISCOVERY_SEED_ROLE_POLICY="${ROBDEX_AGENT_RUNTIME_SEED_ROLE_POLICY:-importSeeds}" \
  ROBDEX_AGENT_RUNTIME_DISCOVERY_COMMAND_BOOTSTRAP_POLICY="${ROBDEX_AGENT_RUNTIME_COMMAND_BOOTSTRAP_POLICY:-bootstrapDefaults}" \
  ROBDEX_AGENT_RUNTIME_DISCOVERY_PROCESS_RECONCILIATION_POLICY="${ROBDEX_AGENT_RUNTIME_PROCESS_RECONCILIATION_POLICY:-markRunningLost}" \
  ROBDEX_AGENT_RUNTIME_DISCOVERY_SHUTDOWN_POLICY="${ROBDEX_AGENT_RUNTIME_SHUTDOWN_POLICY:-gracefulMarkRunningLost}" \
  python3 <<'PY'
import datetime
import json
import os
import signal
import sys
import urllib.error
import urllib.request
from pathlib import Path
from urllib.parse import urlsplit, urlunsplit

def now():
    return datetime.datetime.now(datetime.timezone.utc).isoformat().replace("+00:00", "Z")

def redact(url):
    parts = urlsplit(url)
    netloc = parts.netloc
    if "@" in netloc:
        userinfo, host = netloc.rsplit("@", 1)
        if ":" in userinfo:
            user = userinfo.split(":", 1)[0]
            netloc = f"{user}:***@{host}"
        else:
            netloc = f"{userinfo}@{host}"
    return urlunsplit((parts.scheme, netloc, parts.path, parts.query, parts.fragment))

def read_pid(path):
    try:
        text = Path(path).read_text(encoding="utf-8").strip()
    except FileNotFoundError:
        return None
    if not text:
        return None
    try:
        return int(text)
    except ValueError:
        return text

def pid_alive(pid):
    if not isinstance(pid, int):
        return False
    try:
        os.kill(pid, 0)
        return True
    except OSError:
        return False

def mtime(path):
    try:
        return datetime.datetime.fromtimestamp(Path(path).stat().st_mtime, datetime.timezone.utc).isoformat().replace("+00:00", "Z")
    except FileNotFoundError:
        return None

def health_check(url, should_check):
    checked_at = now()
    if not should_check:
        return {
            "checked": False,
            "ok": None,
            "statusCode": None,
            "body": None,
            "error": None,
            "checkedAt": checked_at,
        }
    try:
        with urllib.request.urlopen(url, timeout=1.5) as response:
            raw = response.read(131072).decode("utf-8", errors="replace")
            body = None
            try:
                body = json.loads(raw)
            except json.JSONDecodeError:
                body = {"raw": raw}
            return {
                "checked": True,
                "ok": 200 <= response.status < 300,
                "statusCode": response.status,
                "body": body,
                "error": None,
                "checkedAt": checked_at,
            }
    except Exception as error:
        return {
            "checked": True,
            "ok": False,
            "statusCode": None,
            "body": None,
            "error": str(error),
            "checkedAt": checked_at,
        }

state_dir = os.environ["ROBDEX_AGENT_RUNTIME_DISCOVERY_STATE_DIR"]
pid_file = os.environ["ROBDEX_AGENT_RUNTIME_DISCOVERY_PID_FILE"]
config_file = os.environ["ROBDEX_AGENT_RUNTIME_DISCOVERY_CONFIG_FILE"]
discovery_file = os.environ["ROBDEX_AGENT_RUNTIME_DISCOVERY_FILE"]
stdout_log = os.environ["ROBDEX_AGENT_RUNTIME_DISCOVERY_STDOUT_LOG"]
stderr_log = os.environ["ROBDEX_AGENT_RUNTIME_DISCOVERY_STDERR_LOG"]
health_url = os.environ["ROBDEX_AGENT_RUNTIME_DISCOVERY_HEALTH_URL"]
database_url = os.environ["ROBDEX_AGENT_RUNTIME_DISCOVERY_DATABASE_URL"]

pid = read_pid(pid_file)
alive = pid_alive(pid)
pid_file_exists = Path(pid_file).exists()
config_exists = Path(config_file).exists()
discovery_exists = Path(discovery_file).exists()
health = health_check(health_url, alive)

missing_config = not config_exists
stale_pid = bool(pid_file_exists and pid is not None and not alive)
running = bool(pid is not None and alive and health["ok"])
unhealthy = bool(pid is not None and alive and health["ok"] is False)
stopped = bool(pid is None and not pid_file_exists)
stale_discovery = False
if discovery_exists:
    discovery_mtime = Path(discovery_file).stat().st_mtime
    if config_exists and discovery_mtime < Path(config_file).stat().st_mtime:
        stale_discovery = True
    if pid_file_exists and discovery_mtime < Path(pid_file).stat().st_mtime:
        stale_discovery = True
else:
    stale_discovery = True

if stale_pid:
    service_state = "stalePid"
elif unhealthy:
    service_state = "unhealthy"
elif running:
    service_state = "running"
elif missing_config:
    service_state = "missingConfig"
else:
    service_state = "stopped"

diagnostics = []
if stopped:
    diagnostics.append({"code": "stopped", "message": "no pid file is present"})
if stale_pid:
    diagnostics.append({"code": "stale_pid", "message": "pid file exists but process is not alive", "pid": pid})
if unhealthy:
    diagnostics.append({"code": "unhealthy", "message": "server process is alive but health check failed", "healthError": health["error"]})
if missing_config:
    diagnostics.append({"code": "missing_config", "message": "effective configuration snapshot is missing", "path": config_file})
if stale_discovery:
    diagnostics.append({"code": "stale_discovery", "message": "discovery file was missing or older than service metadata before this refresh", "path": discovery_file})

runtime_identity = None
if isinstance(health.get("body"), dict):
    runtime_identity = health["body"].get("runtimeIdentity") or health["body"].get("runtime_identity")

packet = {
    "contractVersion": 1,
    "serviceState": service_state,
    "stateFlags": {
        "stopped": stopped,
        "running": running,
        "stalePid": stale_pid,
        "unhealthy": unhealthy,
        "missingConfig": missing_config,
        "staleDiscovery": stale_discovery,
    },
    "baseUrl": os.environ["ROBDEX_AGENT_RUNTIME_DISCOVERY_BASE_URL"],
    "healthUrl": health_url,
    "webSocketUrl": os.environ["ROBDEX_AGENT_RUNTIME_DISCOVERY_WEBSOCKET_URL"],
    "runtimeIdentity": runtime_identity,
    "pid": pid,
    "pidLiveness": {
        "pidFileExists": pid_file_exists,
        "alive": alive,
        "checkedAt": health["checkedAt"],
    },
    "stateDirectory": state_dir,
    "paths": {
        "pidFile": pid_file,
        "configFile": config_file,
        "stdoutLog": stdout_log,
        "stderrLog": stderr_log,
        "discoveryFile": discovery_file,
    },
    "databaseTarget": {
        "urlRedacted": redact(database_url),
    },
    "effectivePolicy": {
        "bindHost": os.environ["ROBDEX_AGENT_RUNTIME_DISCOVERY_HOST"],
        "bindPort": int(os.environ["ROBDEX_AGENT_RUNTIME_DISCOVERY_PORT"]),
        "schemaPolicy": os.environ["ROBDEX_AGENT_RUNTIME_DISCOVERY_SCHEMA_POLICY"],
        "seedRolePolicy": os.environ["ROBDEX_AGENT_RUNTIME_DISCOVERY_SEED_ROLE_POLICY"],
        "commandBootstrapPolicy": os.environ["ROBDEX_AGENT_RUNTIME_DISCOVERY_COMMAND_BOOTSTRAP_POLICY"],
        "processReconciliationPolicy": os.environ["ROBDEX_AGENT_RUNTIME_DISCOVERY_PROCESS_RECONCILIATION_POLICY"],
        "shutdownPolicy": os.environ["ROBDEX_AGENT_RUNTIME_DISCOVERY_SHUTDOWN_POLICY"],
    },
    "healthResult": health,
    "diagnostics": diagnostics,
    "timestamps": {
        "generatedAt": now(),
        "pidFileModifiedAt": mtime(pid_file),
        "configFileModifiedAt": mtime(config_file),
        "discoveryFileModifiedAt": mtime(discovery_file),
        "stdoutLogModifiedAt": mtime(stdout_log),
        "stderrLogModifiedAt": mtime(stderr_log),
    },
}
text = json.dumps(packet, indent=2, sort_keys=True) + "\n"
if os.environ.get("ROBDEX_AGENT_RUNTIME_DISCOVERY_WRITE") == "write":
    Path(discovery_file).write_text(text, encoding="utf-8")
sys.stdout.write(text)
PY
}

write_discovery() {
  discovery_packet write >/dev/null
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
    write_discovery || true
    return 1
  fi
  write_discovery
  printf '[agent-runtime-service] started\n'
  printf 'base_url=%s\n' "$BASE_URL"
  printf 'pid=%s\n' "$pid"
  printf 'pid_file=%s\n' "$PID_FILE"
  printf 'stdout_log=%s\n' "$STDOUT_LOG"
  printf 'stderr_log=%s\n' "$STDERR_LOG"
  printf 'config=%s\n' "$CONFIG_FILE"
  printf 'discovery=%s\n' "$DISCOVERY_FILE"
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
    write_discovery
    return 0
  fi
  if ! is_running_pid "$pid"; then
    printf '[agent-runtime-service] stale pid file removed: %s pid=%s\n' "$PID_FILE" "$pid"
    rm -f "$PID_FILE"
    write_discovery
    return 0
  fi
  printf '[agent-runtime-service] stopping pid=%s\n' "$pid"
  kill "$pid" 2>/dev/null || true
  for ((attempt=1; attempt<=STOP_DEADLINE_SECONDS*10; attempt++)); do
    if ! is_running_pid "$pid"; then
      rm -f "$PID_FILE"
      write_discovery
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
        write_discovery
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
  printf 'discovery=%s\n' "$DISCOVERY_FILE"
  write_discovery
  if [[ "$state" == "stale" ]]; then
    return 2
  fi
}

discover_service() {
  discovery_packet write
}

write_package_descriptor() {
  local server_bin="$1"
  local script_path
  script_path="$(pwd)/scripts/agent-runtime-service.sh"
  local redacted_db
  redacted_db="$(redact_database_url)"
  python3 - "$PACKAGE_FILE" <<PY
import json, os, sys
path = sys.argv[1]
packet = {
    "contractVersion": 1,
    "packageState": "installed",
    "stateDirectory": "$STATE_DIR",
    "discoveryFile": "$DISCOVERY_FILE",
    "configFile": "$CONFIG_FILE",
    "pidFile": "$PID_FILE",
    "stdoutLog": "$STDOUT_LOG",
    "stderrLog": "$STDERR_LOG",
    "serviceScript": "$script_path",
    "serverBinary": "$server_bin",
    "databaseUrlRedacted": "$redacted_db",
    "launchd": {
        "label": "$LAUNCHD_LABEL",
        "plistPath": "$LAUNCHD_PLIST",
        "status": "deferred",
        "reason": "per-user launchd autostart requires a separate owner-approved gate"
    },
    "commands": {
        "start": "$script_path start",
        "stop": "$script_path stop",
        "restart": "$script_path restart",
        "status": "$script_path status",
        "discover": "$script_path discover",
        "logs": "$script_path logs"
    },
    "environmentOverride": "ROBDEX_AGENT_RUNTIME_SERVICE_STATE_DIR",
}
with open(path, "w", encoding="utf-8") as fh:
    json.dump(packet, fh, indent=2, sort_keys=True)
    fh.write("\\n")
print(json.dumps(packet, indent=2, sort_keys=True))
PY
}

install_user_service() {
  ensure_state_dir
  local server_bin
  server_bin="$(build_or_locate_server)"
  write_package_descriptor "$server_bin"
}

uninstall_user_service() {
  stop_service "" >/dev/null || true
  rm -f "$PACKAGE_FILE"
  discover_service >/dev/null
  printf '[agent-runtime-service] user service package removed: %s\n' "$PACKAGE_FILE"
}

package_status() {
  ensure_state_dir
  if [[ -f "$PACKAGE_FILE" ]]; then
    cat "$PACKAGE_FILE"
  else
    python3 - <<PY
import json
print(json.dumps({
    "contractVersion": 1,
    "packageState": "notInstalled",
    "stateDirectory": "$STATE_DIR",
    "discoveryFile": "$DISCOVERY_FILE",
    "packageFile": "$PACKAGE_FILE",
    "launchd": {
        "label": "$LAUNCHD_LABEL",
        "plistPath": "$LAUNCHD_PLIST",
        "status": "deferred"
    }
}, indent=2, sort_keys=True))
PY
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
usage: $0 <start|stop|status|discover|json-status|restart|logs|default-state-dir|install-user-service|uninstall-user-service|package-status> [--force|--tail]

Environment:
  ROBDEX_AGENT_RUNTIME_SERVICE_STATE_DIR   override state directory, default $(default_state_dir)
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
  discover|json-status) discover_service ;;
  restart) shift; restart_service "${1:-}" ;;
  logs) shift; logs_service "${1:-print}" ;;
  default-state-dir) default_state_dir ;;
  install-user-service) install_user_service ;;
  uninstall-user-service) uninstall_user_service ;;
  package-status) package_status ;;
  -h|--help|help|"") usage ;;
  *) usage >&2; exit 2 ;;
esac

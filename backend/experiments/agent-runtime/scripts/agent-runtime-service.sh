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
LAUNCHD_LABEL="${ROBDEX_AGENT_RUNTIME_LAUNCHD_LABEL:-com.robdex.agent-runtime.experimental}"
LAUNCHD_PLIST="$HOME/Library/LaunchAgents/$LAUNCHD_LABEL.plist"
LAUNCHD_STDOUT_LOG="$STATE_DIR/launchd.stdout.log"
LAUNCHD_STDERR_LOG="$STATE_DIR/launchd.stderr.log"
LAUNCHD_DOMAIN="gui/$(id -u)"
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

service_script_path() {
  printf '%s\n' "$(pwd)/scripts/agent-runtime-service.sh"
}

launchd_target() {
  printf '%s/%s\n' "$LAUNCHD_DOMAIN" "$LAUNCHD_LABEL"
}

launchctl_available() {
  command -v launchctl >/dev/null 2>&1
}

launchd_loaded() {
  launchctl_available && launchctl print "$(launchd_target)" >/dev/null 2>&1
}

launchd_status_value() {
  local service_json="$1"
  if [[ ! -f "$LAUNCHD_PLIST" ]]; then
    printf '%s\n' "notInstalled"
  elif ! launchctl_available; then
    printf '%s\n' "staleUnknown"
  elif ! launchd_loaded; then
    printf '%s\n' "installedUnloaded"
  else
    local service_state health_ok
    service_state="$(python3 - "$service_json" <<'PY'
import json, sys
with open(sys.argv[1], encoding="utf-8") as fh:
    packet = json.load(fh)
print(packet.get("serviceState") or "")
PY
)"
    health_ok="$(python3 - "$service_json" <<'PY'
import json, sys
with open(sys.argv[1], encoding="utf-8") as fh:
    packet = json.load(fh)
ok = (packet.get("healthResult") or {}).get("ok")
print("true" if ok is True else "false" if ok is False else "")
PY
)"
    if [[ "$service_state" == "running" && "$health_ok" == "true" ]]; then
      printf '%s\n' "loadedRunning"
    elif [[ "$service_state" == "unhealthy" || "$health_ok" == "false" ]]; then
      printf '%s\n' "loadedUnhealthy"
    else
      printf '%s\n' "staleUnknown"
    fi
  fi
}

launchd_status_packet() {
  ensure_state_dir
  local service_json="$STATE_DIR/launchd-service-state.json"
  discovery_packet write >"$service_json"
  local status
  status="$(launchd_status_value "$service_json")"
  local loaded="false"
  launchd_loaded && loaded="true"
  local loaded_py="False"
  [[ "$loaded" == "true" ]] && loaded_py="True"
  local launchctl_path=""
  if launchctl_available; then
    launchctl_path="$(command -v launchctl)"
  fi
  local launchctl_available_py="False"
  [[ -n "$launchctl_path" ]] && launchctl_available_py="True"
  local plist_installed_py="False"
  [[ -f "$LAUNCHD_PLIST" ]] && plist_installed_py="True"
  local diagnostic=""
  if [[ -f "$LAUNCHD_PLIST" && -z "$launchctl_path" ]]; then
    diagnostic="launchctl is unavailable; load/unload/status cannot verify user-domain state"
  elif [[ "$status" == "installedUnloaded" ]]; then
    diagnostic="plist is installed but launchctl does not report the user-domain job as loaded"
  elif [[ "$status" == "staleUnknown" ]]; then
    diagnostic="launchd state or service health is not conclusively running"
  fi
  python3 - "$service_json" <<PY
import json
import sys
with open(sys.argv[1], encoding="utf-8") as fh:
    service = json.load(fh)
packet = {
    "contractVersion": 1,
    "label": "$LAUNCHD_LABEL",
    "domain": "$LAUNCHD_DOMAIN",
    "target": "$LAUNCHD_DOMAIN/$LAUNCHD_LABEL",
    "plistPath": "$LAUNCHD_PLIST",
    "plistInstalled": $plist_installed_py,
    "loaded": $loaded_py,
    "status": "$status",
    "launchctlAvailable": $launchctl_available_py,
    "launchctlPath": "$launchctl_path" or None,
    "stateDirectory": "$STATE_DIR",
    "discoveryFile": "$DISCOVERY_FILE",
    "stdoutLog": "$LAUNCHD_STDOUT_LOG",
    "stderrLog": "$LAUNCHD_STDERR_LOG",
    "diagnostic": "$diagnostic" or None,
    "service": {
        "serviceState": service.get("serviceState"),
        "healthResult": service.get("healthResult"),
        "pid": service.get("pid"),
        "pidLiveness": service.get("pidLiveness"),
    },
}
print(json.dumps(packet, indent=2, sort_keys=True))
PY
}

write_launchd_plist() {
  ensure_state_dir
  mkdir -p "$(dirname "$LAUNCHD_PLIST")"
  local script_path
  script_path="$(service_script_path)"
  python3 - "$LAUNCHD_PLIST" "$LAUNCHD_LABEL" "$script_path" "$STATE_DIR" "$LAUNCHD_STDOUT_LOG" "$LAUNCHD_STDERR_LOG" "$HOST" "$PORT" <<'PY'
import os
import plistlib
import sys
plist_path, label, script_path, state_dir, stdout_log, stderr_log, host, port = sys.argv[1:9]
env = {
    "ROBDEX_AGENT_RUNTIME_SERVICE_STATE_DIR": state_dir,
    "ROBDEX_AGENT_RUNTIME_SERVER_HOST": host,
    "ROBDEX_AGENT_RUNTIME_SERVER_PORT": port,
}
for name in [
    "ROBDEX_AGENT_RUNTIME_DATABASE_URL",
    "ROBDEX_AGENT_RUNTIME_SERVER_BIN",
    "ROBDEX_AGENT_RUNTIME_IDENTITY",
    "ROBDEX_AGENT_RUNTIME_SCHEMA_POLICY",
    "ROBDEX_AGENT_RUNTIME_SEED_ROLE_POLICY",
    "ROBDEX_AGENT_RUNTIME_COMMAND_BOOTSTRAP_POLICY",
    "ROBDEX_AGENT_RUNTIME_PROCESS_RECONCILIATION_POLICY",
    "ROBDEX_AGENT_RUNTIME_SHUTDOWN_POLICY",
]:
    value = os.environ.get(name)
    if value:
        env[name] = value
packet = {
    "Label": label,
    "ProgramArguments": [script_path, "start"],
    "RunAtLoad": True,
    "WorkingDirectory": os.getcwd(),
    "EnvironmentVariables": env,
    "StandardOutPath": stdout_log,
    "StandardErrorPath": stderr_log,
}
with open(plist_path, "wb") as fh:
    plistlib.dump(packet, fh, sort_keys=True)
print(plist_path)
PY
}

write_package_descriptor() {
  local server_bin="$1"
  local script_path
  script_path="$(service_script_path)"
  local redacted_db
  redacted_db="$(redact_database_url)"
  local launchd_status_json="$STATE_DIR/package-launchd-status.json"
  launchd_status_packet >"$launchd_status_json"
  python3 - "$PACKAGE_FILE" <<PY
import json, os, sys
path = sys.argv[1]
with open("$launchd_status_json", encoding="utf-8") as fh:
    launchd = json.load(fh)
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
    "launchd": launchd,
    "commands": {
        "start": "$script_path start",
        "stop": "$script_path stop",
        "restart": "$script_path restart",
        "status": "$script_path status",
        "discover": "$script_path discover",
        "logs": "$script_path logs",
        "installLaunchd": "$script_path install-launchd",
        "loadLaunchd": "$script_path load-launchd",
        "unloadLaunchd": "$script_path unload-launchd",
        "uninstallLaunchd": "$script_path uninstall-launchd",
        "launchdStatus": "$script_path launchd-status"
    },
    "environmentOverride": "ROBDEX_AGENT_RUNTIME_SERVICE_STATE_DIR",
}
with open(path, "w", encoding="utf-8") as fh:
    json.dump(packet, fh, indent=2, sort_keys=True)
    fh.write("\\n")
print(json.dumps(packet, indent=2, sort_keys=True))
PY
}

package_server_bin_no_build() {
  if [[ -f "$PACKAGE_FILE" ]]; then
    python3 - "$PACKAGE_FILE" <<'PY'
import json
import sys
try:
    with open(sys.argv[1], encoding="utf-8") as fh:
        packet = json.load(fh)
except FileNotFoundError:
    packet = {}
print(packet.get("serverBinary") or "")
PY
  elif [[ -n "${ROBDEX_AGENT_RUNTIME_SERVER_BIN:-}" ]]; then
    printf '%s\n' "$ROBDEX_AGENT_RUNTIME_SERVER_BIN"
  elif [[ -x target/debug/robdex-agent-runtime-server ]]; then
    printf '%s\n' "target/debug/robdex-agent-runtime-server"
  else
    printf '%s\n' ""
  fi
}

install_user_service() {
  ensure_state_dir
  local server_bin
  server_bin="$(build_or_locate_server)"
  write_package_descriptor "$server_bin"
}

uninstall_user_service() {
  if [[ -f "$LAUNCHD_PLIST" ]]; then
    uninstall_launchd >/dev/null || true
  else
    stop_service "" >/dev/null || true
  fi
  rm -f "$PACKAGE_FILE"
  discover_service >/dev/null
  printf '[agent-runtime-service] user service package removed: %s\n' "$PACKAGE_FILE"
}

package_status() {
  ensure_state_dir
  local launchd_status_json="$STATE_DIR/package-launchd-status.json"
  launchd_status_packet >"$launchd_status_json"
  if [[ -f "$PACKAGE_FILE" ]]; then
    python3 - "$PACKAGE_FILE" "$launchd_status_json" <<'PY'
import json
import sys
with open(sys.argv[1], encoding="utf-8") as fh:
    packet = json.load(fh)
with open(sys.argv[2], encoding="utf-8") as fh:
    packet["launchd"] = json.load(fh)
print(json.dumps(packet, indent=2, sort_keys=True))
PY
  else
    python3 - "$launchd_status_json" <<PY
import json
import sys
with open(sys.argv[1], encoding="utf-8") as fh:
    launchd = json.load(fh)
print(json.dumps({
    "contractVersion": 1,
    "packageState": "notInstalled",
    "stateDirectory": "$STATE_DIR",
    "discoveryFile": "$DISCOVERY_FILE",
    "packageFile": "$PACKAGE_FILE",
    "launchd": launchd
}, indent=2, sort_keys=True))
PY
  fi
}

install_launchd() {
  ensure_state_dir
  install_user_service >/dev/null
  local plist
  plist="$(write_launchd_plist)"
  local server_bin
  server_bin="$(build_or_locate_server)"
  write_package_descriptor "$server_bin" >/dev/null
  printf '[agent-runtime-service] launchd plist installed: %s\n' "$plist" >&2
  launchd_status_packet
}

load_launchd() {
  if ! launchctl_available; then
    printf '[agent-runtime-service] launchctl is unavailable; refusing to claim launchd active\n' >&2
    launchd_status_packet >&2 || true
    return 1
  fi
  if [[ ! -f "$LAUNCHD_PLIST" ]]; then
    install_launchd >/dev/null
  fi
  if launchd_loaded; then
    printf '[agent-runtime-service] launchd already loaded: %s\n' "$(launchd_target)" >&2
    launchd_status_packet
    return 0
  fi
  if launchctl bootstrap "$LAUNCHD_DOMAIN" "$LAUNCHD_PLIST"; then
    printf '[agent-runtime-service] launchd loaded: %s\n' "$(launchd_target)" >&2
    launchd_status_packet
  else
    local status=$?
    printf '[agent-runtime-service] launchctl bootstrap failed for %s\n' "$(launchd_target)" >&2
    launchd_status_packet >&2 || true
    return "$status"
  fi
}

unload_launchd() {
  if ! launchctl_available; then
    printf '[agent-runtime-service] launchctl is unavailable; refusing to claim launchd unloaded\n' >&2
    launchd_status_packet >&2 || true
    return 1
  fi
  local status=0
  if [[ -f "$LAUNCHD_PLIST" ]]; then
    if launchd_loaded; then
      launchctl bootout "$LAUNCHD_DOMAIN" "$LAUNCHD_PLIST" || status=$?
    fi
  fi
  stop_service "" >/dev/null || status=$?
  launchd_status_packet
  return "$status"
}

uninstall_launchd() {
  unload_launchd >/dev/null || true
  rm -f "$LAUNCHD_PLIST"
  local server_bin
  server_bin="$(package_server_bin_no_build)"
  write_package_descriptor "$server_bin" >/dev/null
  printf '[agent-runtime-service] launchd plist removed: %s\n' "$LAUNCHD_PLIST" >&2
  launchd_status_packet
}

launchd_status_service() {
  launchd_status_packet
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
usage: $0 <start|stop|status|discover|json-status|restart|logs|default-state-dir|install-user-service|uninstall-user-service|package-status|install-launchd|load-launchd|unload-launchd|uninstall-launchd|launchd-status> [--force|--tail]

Environment:
  ROBDEX_AGENT_RUNTIME_SERVICE_STATE_DIR   override state directory, default $(default_state_dir)
  ROBDEX_AGENT_RUNTIME_DATABASE_URL        runtime Postgres URL
  ROBDEX_AGENT_RUNTIME_SERVER_HOST         bind host, default 127.0.0.1
  ROBDEX_AGENT_RUNTIME_SERVER_PORT         bind port, default 8765
  ROBDEX_AGENT_RUNTIME_SERVER_BIN          optional existing server binary
  ROBDEX_AGENT_RUNTIME_LAUNCHD_LABEL       launchd label, default $LAUNCHD_LABEL
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
  install-launchd) install_launchd ;;
  load-launchd) load_launchd ;;
  unload-launchd) unload_launchd ;;
  uninstall-launchd) uninstall_launchd ;;
  launchd-status) launchd_status_service ;;
  -h|--help|help|"") usage ;;
  *) usage >&2; exit 2 ;;
esac

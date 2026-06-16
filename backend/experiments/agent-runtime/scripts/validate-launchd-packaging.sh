#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

VALIDATION_ROOT="$(mktemp -d /tmp/robdex-agent-runtime-launchd.XXXXXX)"
cleanup() {
  rm -rf "$VALIDATION_ROOT"
}
trap cleanup EXIT

export HOME="$VALIDATION_ROOT/home"
export ROBDEX_AGENT_RUNTIME_SERVICE_STATE_DIR="$VALIDATION_ROOT/state"
export ROBDEX_AGENT_RUNTIME_LAUNCHD_LABEL="com.robdex.agent-runtime.validation.$$"
export ROBDEX_AGENT_RUNTIME_SERVER_BIN="/bin/echo"
export ROBDEX_AGENT_RUNTIME_SERVER_HOST="127.0.0.1"
export ROBDEX_AGENT_RUNTIME_SERVER_PORT="8765"
mkdir -p "$HOME"

SERVICE="scripts/agent-runtime-service.sh"
PLIST="$HOME/Library/LaunchAgents/$ROBDEX_AGENT_RUNTIME_LAUNCHD_LABEL.plist"
DISCOVERY="$ROBDEX_AGENT_RUNTIME_SERVICE_STATE_DIR/discovery.json"
PACKAGE="$ROBDEX_AGENT_RUNTIME_SERVICE_STATE_DIR/service-package.json"

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
    printf '[launchd-validation] expected %s=%s in %s, got %s\n' "$expr" "$expected" "$path" "$actual" >&2
    exit 1
  fi
}

DEFAULT_STATE_DIR="$(/usr/bin/env -u ROBDEX_AGENT_RUNTIME_SERVICE_STATE_DIR "$SERVICE" default-state-dir)"
case "$DEFAULT_STATE_DIR" in
  "$PWD"/*|"$PWD/.runtime-service"|.runtime-service|*/backend/experiments/agent-runtime/.runtime-service)
    printf '[launchd-validation] default state dir is not user-scoped: %s\n' "$DEFAULT_STATE_DIR" >&2
    exit 1
    ;;
esac
printf '[launchd-validation] default_state_dir=%s\n' "$DEFAULT_STATE_DIR"

printf '[launchd-validation] launchd status before install\n'
$SERVICE launchd-status >"$VALIDATION_ROOT/launchd-not-installed.json"
assert_json_eq "$VALIDATION_ROOT/launchd-not-installed.json" "status" "notInstalled"
assert_json_eq "$VALIDATION_ROOT/launchd-not-installed.json" "plistInstalled" "false"
assert_json_eq "$VALIDATION_ROOT/launchd-not-installed.json" "stateDirectory" "$ROBDEX_AGENT_RUNTIME_SERVICE_STATE_DIR"

printf '[launchd-validation] package status before install\n'
$SERVICE package-status >"$VALIDATION_ROOT/package-not-installed.json"
assert_json_eq "$VALIDATION_ROOT/package-not-installed.json" "packageState" "notInstalled"
assert_json_eq "$VALIDATION_ROOT/package-not-installed.json" "launchd.status" "notInstalled"

printf '[launchd-validation] install launchd plist\n'
$SERVICE install-launchd >"$VALIDATION_ROOT/install-launchd.out"
if [[ ! -s "$PLIST" ]]; then
  printf '[launchd-validation] plist was not generated: %s\n' "$PLIST" >&2
  exit 1
fi
if [[ ! -s "$PACKAGE" ]]; then
  printf '[launchd-validation] package descriptor was not generated: %s\n' "$PACKAGE" >&2
  exit 1
fi
if [[ ! -s "$DISCOVERY" ]]; then
  printf '[launchd-validation] discovery packet was not generated: %s\n' "$DISCOVERY" >&2
  exit 1
fi

python3 - "$PLIST" "$ROBDEX_AGENT_RUNTIME_LAUNCHD_LABEL" "$ROBDEX_AGENT_RUNTIME_SERVICE_STATE_DIR" <<'PY'
import plistlib, sys
plist_path, label, state_dir = sys.argv[1:4]
with open(plist_path, "rb") as fh:
    plist = plistlib.load(fh)
assert plist["Label"] == label, plist
assert plist["ProgramArguments"][1] == "start", plist
assert plist["EnvironmentVariables"]["ROBDEX_AGENT_RUNTIME_SERVICE_STATE_DIR"] == state_dir, plist
assert plist["StandardOutPath"].startswith(state_dir), plist
assert plist["StandardErrorPath"].startswith(state_dir), plist
assert plist["RunAtLoad"] is True, plist
assert "/Library/LaunchDaemons/" not in plist_path, plist_path
PY

$SERVICE launchd-status >"$VALIDATION_ROOT/launchd-installed.json"
assert_json_eq "$VALIDATION_ROOT/launchd-installed.json" "status" "installedUnloaded"
assert_json_eq "$VALIDATION_ROOT/launchd-installed.json" "plistInstalled" "true"
assert_json_eq "$VALIDATION_ROOT/launchd-installed.json" "loaded" "false"
assert_json_eq "$VALIDATION_ROOT/launchd-installed.json" "plistPath" "$PLIST"

$SERVICE package-status >"$VALIDATION_ROOT/package-installed.json"
assert_json_eq "$VALIDATION_ROOT/package-installed.json" "packageState" "installed"
assert_json_eq "$VALIDATION_ROOT/package-installed.json" "launchd.status" "installedUnloaded"
assert_json_eq "$VALIDATION_ROOT/package-installed.json" "launchd.plistPath" "$PLIST"
assert_json_eq "$VALIDATION_ROOT/package-installed.json" "discoveryFile" "$DISCOVERY"

printf '[launchd-validation] mocked launchctl lifecycle\n'
STUB_BIN="$VALIDATION_ROOT/bin"
STUB_STATE="$VALIDATION_ROOT/launchctl-state"
mkdir -p "$STUB_BIN"
/bin/cat >"$STUB_BIN/launchctl" <<'SH'
#!/usr/bin/env bash
set -euo pipefail
state="${LAUNCHCTL_STUB_STATE:?}"
log="${LAUNCHCTL_STUB_LOG:?}"
printf '%s\n' "$*" >>"$log"
case "${1:-}" in
  print)
    [[ -f "$state/loaded" ]]
    ;;
  bootstrap)
    if [[ -f "$state/loaded" ]]; then
      exit 37
    fi
    mkdir -p "$state"
    printf '%s\n' "${3:-}" >"$state/loaded"
    ;;
  bootout)
    rm -f "$state/loaded"
    ;;
  *)
    exit 64
    ;;
esac
SH
chmod +x "$STUB_BIN/launchctl"
mkdir -p "$STUB_STATE"
export LAUNCHCTL_STUB_STATE="$STUB_STATE"
export LAUNCHCTL_STUB_LOG="$VALIDATION_ROOT/launchctl.log"
export PATH="$STUB_BIN:$PATH"

rm -f "$LAUNCHCTL_STUB_LOG" "$STUB_STATE/loaded"
$SERVICE load-launchd >"$VALIDATION_ROOT/load-launchd.json"
assert_json_eq "$VALIDATION_ROOT/load-launchd.json" "loaded" "true"
assert_json_eq "$VALIDATION_ROOT/load-launchd.json" "status" "staleUnknown"
if ! python3 - "$LAUNCHCTL_STUB_LOG" <<'PY'
import sys
text = open(sys.argv[1], encoding="utf-8").read()
sys.exit(0 if any(line.startswith("bootstrap ") for line in text.splitlines()) else 1)
PY
then
  printf '[launchd-validation] mocked bootstrap was not called for initial load\n' >&2
  exit 1
fi

: >"$LAUNCHCTL_STUB_LOG"
$SERVICE load-launchd >"$VALIDATION_ROOT/load-launchd-already.json"
assert_json_eq "$VALIDATION_ROOT/load-launchd-already.json" "loaded" "true"
if python3 - "$LAUNCHCTL_STUB_LOG" <<'PY'
import sys
text = open(sys.argv[1], encoding="utf-8").read()
sys.exit(0 if any(line.startswith("bootstrap ") for line in text.splitlines()) else 1)
PY
then
  printf '[launchd-validation] idempotent load called bootstrap for an already-loaded job\n' >&2
  /bin/cat "$LAUNCHCTL_STUB_LOG" >&2
  exit 1
fi

$SERVICE unload-launchd >"$VALIDATION_ROOT/unload-launchd.json"
assert_json_eq "$VALIDATION_ROOT/unload-launchd.json" "loaded" "false"
assert_json_eq "$VALIDATION_ROOT/unload-launchd.json" "status" "installedUnloaded"
if [[ -f "$STUB_STATE/loaded" ]]; then
  printf '[launchd-validation] mocked bootout did not clear loaded state\n' >&2
  exit 1
fi

$SERVICE uninstall-launchd >"$VALIDATION_ROOT/uninstall-launchd.json"
assert_json_eq "$VALIDATION_ROOT/uninstall-launchd.json" "status" "notInstalled"
if [[ -e "$PLIST" ]]; then
  printf '[launchd-validation] uninstall-launchd did not remove plist\n' >&2
  exit 1
fi

export ROBDEX_AGENT_RUNTIME_SERVER_BIN="$VALIDATION_ROOT/missing-server-bin"
$SERVICE uninstall-user-service >"$VALIDATION_ROOT/uninstall-user-service.out"
if [[ -f "$PACKAGE" ]]; then
  printf '[launchd-validation] uninstall-user-service did not remove package descriptor\n' >&2
  exit 1
fi

printf '[launchd-validation] deterministic launchd packaging validation complete\n'

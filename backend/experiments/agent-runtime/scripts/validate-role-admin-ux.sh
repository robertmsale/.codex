#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR/.."
source "$SCRIPT_DIR/lib-validation-db.sh"
validation_setup_database

run() {
  printf '\n$ %s\n' "$*"
  "$@"
}

sql() {
  psql "$ROBDEX_AGENT_RUNTIME_DATABASE_URL" -Atq -c "$1"
}

assert_eq() {
  local label="$1" expected="$2" actual="$3"
  if [[ "$expected" != "$actual" ]]; then
    printf '%s expected %s got %s\n' "$label" "$expected" "$actual" >&2
    exit 1
  fi
}

run cargo run --quiet -- init-db
run cargo run --quiet -- roles import-seeds

TMPDIR="$(mktemp -d /tmp/agent-runtime-role-admin.XXXXXX)"
mkdir -p "$TMPDIR/prompts"
cp roles/prompts/runtime-allow.md "$TMPDIR/prompts/runtime-admin.md"

python3 - "$TMPDIR/runtime-admin.json" <<'PY'
import json, pathlib, sys
d=json.load(open("roles/runtime-allow.json"))
d["id"]="runtime-admin"
d["version"]="1.0.0"
d["displayName"]="Runtime Admin"
d["prompt"]["path"]="prompts/runtime-admin.md"
pathlib.Path(sys.argv[1]).write_text(json.dumps(d, indent=2) + "\n")
PY

run cargo run --quiet -- roles validate --manifest "$TMPDIR/runtime-admin.json"
run cargo run --quiet -- roles create "$TMPDIR/runtime-admin.json"
CREATE_VERSION_COUNT_BEFORE="$(sql "select count(*) from role_versions where role_id='runtime-admin'")"
set +e
DUP_CREATE="$(cargo run --quiet -- roles create "$TMPDIR/runtime-admin.json" 2>&1)"
DUP_CREATE_STATUS=$?
set -e
printf 'duplicate_create_status=%s\n%s\n' "$DUP_CREATE_STATUS" "$DUP_CREATE"
[[ "$DUP_CREATE_STATUS" -ne 0 ]]
printf '%s\n' "$DUP_CREATE" | rg 'role already exists'
CREATE_VERSION_COUNT_AFTER="$(sql "select count(*) from role_versions where role_id='runtime-admin'")"
assert_eq duplicate_create_no_version "$CREATE_VERSION_COUNT_BEFORE" "$CREATE_VERSION_COUNT_AFTER"
LIST_OUT="$(cargo run --quiet -- roles list)"
printf '%s\n' "$LIST_OUT" | rg '"id":"runtime-admin"'
SHOW_V1="$(cargo run --quiet -- roles show runtime-admin)"
printf '%s\n' "$SHOW_V1" | rg '"instructionText"'
OLD_SESSION="$(cargo run --quiet -- sessions new --role runtime-admin)"
OLD_SNAPSHOT_VERSION="$(sql "select role_snapshot->>'version' from sessions where id='$OLD_SESSION'")"
assert_eq old_snapshot_initial "1.0.0" "$OLD_SNAPSHOT_VERSION"

python3 - "$TMPDIR/runtime-admin.json" <<'PY'
import json, pathlib, sys
path=pathlib.Path(sys.argv[1])
d=json.loads(path.read_text())
d["version"]="2.0.0"
d["displayName"]="Runtime Admin V2"
path.write_text(json.dumps(d, indent=2) + "\n")
PY
printf '\nupdated admin prompt\n' >> "$TMPDIR/prompts/runtime-admin.md"
run cargo run --quiet -- roles update "$TMPDIR/runtime-admin.json"
python3 - "$TMPDIR/runtime-missing-update.json" <<'PY'
import json, pathlib, sys
d=json.load(open("roles/runtime-allow.json"))
d["id"]="runtime-missing-update"
d["version"]="1.0.0"
d["displayName"]="Runtime Missing Update"
d["prompt"]["path"]="prompts/runtime-admin.md"
pathlib.Path(sys.argv[1]).write_text(json.dumps(d, indent=2) + "\n")
PY
set +e
MISSING_UPDATE="$(cargo run --quiet -- roles update "$TMPDIR/runtime-missing-update.json" 2>&1)"
MISSING_UPDATE_STATUS=$?
set -e
printf 'missing_update_status=%s\n%s\n' "$MISSING_UPDATE_STATUS" "$MISSING_UPDATE"
[[ "$MISSING_UPDATE_STATUS" -ne 0 ]]
printf '%s\n' "$MISSING_UPDATE" | rg 'role does not exist'
MISSING_UPDATE_COUNT="$(sql "select count(*) from roles where id='runtime-missing-update'")"
assert_eq update_does_not_create_missing "0" "$MISSING_UPDATE_COUNT"
MISSING_UPDATE_VERSION_COUNT="$(sql "select count(*) from role_versions where role_id='runtime-missing-update'")"
assert_eq update_does_not_create_missing_version "0" "$MISSING_UPDATE_VERSION_COUNT"
NEW_SESSION="$(cargo run --quiet -- sessions new --role runtime-admin)"
NEW_SNAPSHOT_VERSION="$(sql "select role_snapshot->>'version' from sessions where id='$NEW_SESSION'")"
OLD_SNAPSHOT_AFTER_UPDATE="$(sql "select role_snapshot->>'version' from sessions where id='$OLD_SESSION'")"
assert_eq hot_future_session_version "2.0.0" "$NEW_SNAPSHOT_VERSION"
assert_eq existing_session_immutable "1.0.0" "$OLD_SNAPSHOT_AFTER_UPDATE"

VERSIONS_JSON="$(cargo run --quiet -- roles versions runtime-admin)"
printf '%s\n' "$VERSIONS_JSON" | rg '"version": "1.0.0"'
printf '%s\n' "$VERSIONS_JSON" | rg '"version": "2.0.0"'
V1_ID="$(printf '%s\n' "$VERSIONS_JSON" | python3 -c 'import json,sys; rows=json.load(sys.stdin); print([r["roleVersionId"] for r in rows if r["version"]=="1.0.0"][0])')"
V2_ID="$(printf '%s\n' "$VERSIONS_JSON" | python3 -c 'import json,sys; rows=json.load(sys.stdin); print([r["roleVersionId"] for r in rows if r["version"]=="2.0.0"][0])')"
run cargo run --quiet -- roles version "$V1_ID"
run cargo run --quiet -- roles activate runtime-admin --version-id "$V1_ID"
ROLLBACK_SESSION="$(cargo run --quiet -- sessions new --role runtime-admin)"
ROLLBACK_VERSION="$(sql "select role_snapshot->>'version' from sessions where id='$ROLLBACK_SESSION'")"
assert_eq rollback_future_session_version "1.0.0" "$ROLLBACK_VERSION"
VERSION_COUNT_AFTER_ROLLBACK="$(sql "select count(*) from role_versions where role_id='runtime-admin'")"
assert_eq rollback_preserves_history "2" "$VERSION_COUNT_AFTER_ROLLBACK"

EXPORT_FILE="$TMPDIR/runtime-admin-export.json"
run cargo run --quiet -- roles activate runtime-admin --version-id "$V2_ID"
run cargo run --quiet -- roles export runtime-admin --out "$EXPORT_FILE"
test -s "$EXPORT_FILE"
rm -rf "$TMPDIR/prompts"
run cargo run --quiet -- roles import "$EXPORT_FILE"
IMPORT_COUNT="$(sql "select count(*) from role_versions where role_id='runtime-admin'")"
[[ "$IMPORT_COUNT" -ge 3 ]]

run cargo run --quiet -- roles archive runtime-admin
ARCHIVED_STATUS="$(sql "select status from roles where id='runtime-admin'")"
assert_eq archived_status "archived" "$ARCHIVED_STATUS"
set +e
ARCHIVED_NEW="$(cargo run --quiet -- sessions new --role runtime-admin 2>&1)"
ARCHIVED_NEW_STATUS=$?
set -e
printf 'archived_new_status=%s\n%s\n' "$ARCHIVED_NEW_STATUS" "$ARCHIVED_NEW"
printf '%s\n' "$ARCHIVED_NEW" | rg 'no rows returned|not found'
run cargo run --quiet -- roles unarchive runtime-admin
UNARCHIVE_VERSION_COUNT="$(sql "select count(*) from role_versions where role_id='runtime-admin'")"
assert_eq unarchive_no_new_version "$IMPORT_COUNT" "$UNARCHIVE_VERSION_COUNT"
UNARCHIVED_SESSION="$(cargo run --quiet -- sessions new --role runtime-admin)"
printf 'unarchived_session=%s\n' "$UNARCHIVED_SESSION"

make_invalid() {
  local name="$1" py="$2"
  local path="$TMPDIR/$name.json"
  mkdir -p "$TMPDIR/bad-prompts"
  cp roles/prompts/runtime-allow.md "$TMPDIR/bad-prompts/role.md"
  python3 - "$path" "$py" <<'PY'
import json, pathlib, sys
path=pathlib.Path(sys.argv[1])
mode=sys.argv[2]
d=json.load(open("roles/runtime-allow.json"))
d["id"]="runtime-invalid-" + mode.replace("_","-")
d["prompt"]["path"]="bad-prompts/role.md"
if mode=="empty_prompt":
    pathlib.Path(path.parent / "bad-prompts" / "role.md").write_text("")
elif mode=="invalid_id":
    d["id"]="Runtime_Invalid"
elif mode=="missing_model":
    d["modelDefaults"]["model"]=""
elif mode=="unknown_action":
    d["capabilities"].append("unknown.action")
    d["policy"]["unknown.action"]="allow"
elif mode=="concrete_cmd":
    d["capabilities"].append("cmd.rg.run")
    d["policy"]["cmd.rg.run"]="allow"
elif mode=="cap_mismatch":
    d["policy"].pop("fs.read", None)
elif mode=="bad_routing_mode":
    d["routing"]["mode"]="broadcast"
elif mode=="bad_reserved":
    d["routing"]["reservedActions"].append("unknown.reserved")
elif mode=="bad_recipient":
    d["routing"]["defaultRecipient"]="missing-recipient"
    d["routing"]["allowedRecipients"]=["missing-recipient"]
path.write_text(json.dumps(d, indent=2) + "\n")
PY
  printf '%s' "$path"
}

printf '{bad json\n' > "$TMPDIR/invalid-json.json"
for mode in invalid_json empty_prompt invalid_id missing_model unknown_action concrete_cmd cap_mismatch bad_routing_mode bad_reserved bad_recipient; do
  if [[ "$mode" == "invalid_json" ]]; then
    bad="$TMPDIR/invalid-json.json"
  else
    bad="$(make_invalid "$mode" "$mode")"
  fi
  set +e
  OUT="$(cargo run --quiet -- roles validate --manifest "$bad" 2>&1)"
  STATUS=$?
  set -e
  printf 'validation_%s_status=%s\n%s\n' "$mode" "$STATUS" "$OUT"
  [[ "$STATUS" -ne 0 ]]
  printf '%s\n' "$OUT" | rg '"valid": false'
  case "$mode" in
    invalid_json) printf '%s\n' "$OUT" | rg 'role manifest is not valid JSON' ;;
    empty_prompt) printf '%s\n' "$OUT" | rg 'prompt instruction body must not be empty' ;;
    invalid_id) printf '%s\n' "$OUT" | rg 'role id must use lowercase letters' ;;
    missing_model) printf '%s\n' "$OUT" | rg 'modelDefaults.model' ;;
    unknown_action) printf '%s\n' "$OUT" | rg 'unknown action in role manifest: unknown.action' ;;
    concrete_cmd) printf '%s\n' "$OUT" | rg 'concrete command actions are not valid role policy entries: cmd.rg.run' ;;
    cap_mismatch) printf '%s\n' "$OUT" | rg 'capabilities must exactly match policy keys' ;;
    bad_routing_mode) printf '%s\n' "$OUT" | rg 'unsupported routing mode: broadcast' ;;
    bad_reserved) printf '%s\n' "$OUT" | rg 'unknown action in role manifest: unknown.reserved' ;;
    bad_recipient) printf '%s\n' "$OUT" | rg 'invalid routing recipient: missing-recipient' ;;
  esac
done

printf '\n[role admin events]\n'
sql "select event_type || ':' || coalesce(status,'') from event_stream where entity_type='role' order by sequence" | tee "$TMPDIR/events.txt"
for event in role.created role.updated role.imported role.activated role.exported role.archived role.unarchived role.validationFailed role.validationSucceeded; do
  rg "$event" "$TMPDIR/events.txt"
done

run cargo run --quiet -- roles list
run cargo run --quiet -- roles show runtime-admin
run cargo run --quiet -- roles versions runtime-admin
run cargo run --quiet -- roles version "$V2_ID"

printf '\nrole admin UX validation passed\n'

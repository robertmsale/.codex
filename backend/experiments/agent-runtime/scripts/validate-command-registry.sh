#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
source scripts/lib-validation-db.sh
validation_setup_database
run() { printf '\n$ %s\n' "$*"; "$@"; }
sql() { psql "$ROBDEX_AGENT_RUNTIME_DATABASE_URL" -At -c "$1"; }
write_request() {
  local file="$1" operation="$2" action="$3" object="$4" mutation_class="${5:-readOnly}"
  python3 - "$file" "$operation" "$action" "$object" "$mutation_class" <<'PY'
import json, sys
file, operation, action, obj, mutation_class = sys.argv[1:]
req={
  "operation": operation,
  "rationale": f"validation {operation} for {action}",
  "recommendedPolicy":"allow for validation role only",
  "requester":"validation-script",
  "command":{
    "actionId":action,
    "binaryName":"rg",
    "candidatePaths":["/opt/homebrew/bin/rg","/usr/local/bin/rg","/usr/bin/rg"],
    "starlarkObject":obj,
    "starlarkMethod":"run",
    "argvPrefix":["--files"],
    "allowArgsArg":True,
    "allowCwdArg":True,
    "defaultCwd":".",
    "cwdPolicy":"underExecutionRoot",
    "envPolicy":"empty",
    "timeoutMs":5000,
    "outputLimitBytes":12000,
    "mutationClass":mutation_class,
    "modelDescription":f"validation helper for {action}"
  }
}
open(file,'w').write(json.dumps(req))
PY
}

run cargo run --quiet -- init-db
run cargo run --quiet -- roles import-seeds
ADMIN_SESSION=$(cargo run --quiet -- new-session --role runtime-allow)
printf 'admin_session=%s\n' "$ADMIN_SESSION"
printf '\n[seeded registry]\n'; cargo run --quiet -- command-registry list | tee /tmp/agent-runtime-command-registry-list.json
sql "select action_id || ':' || (current_version_id is not null) from command_definitions order by action_id"
printf 'seed_version_trace_columns='; sql "select count(*) from command_versions where action_id in ('cmd.rg.run','cmd.git.status','cmd.git.diff','cmd.cargo.check')"
ALLOW=$(cargo run --quiet -- new-session --role runtime-allow)
printf 'allow_session=%s\n' "$ALLOW"
run cargo run --quiet -- --workdir . send --session "$ALLOW" --message 'Use execute_code with exactly this Starlark source: matches = cmd["rg"].run(args=["--files", "-g", "Cargo.toml"], cwd="."); output(matches)'
printf 'command_version_id_count='; sql "select count(*) from command_runs where command_version_id is not null"
[[ "$(sql "select count(*) from command_runs where command_version_id is not null")" -gt 0 ]]
printf 'command_event_versions='; sql "select count(*) from event_stream where session_id='$ALLOW' and event_type='command.completed' and payload ? 'commandVersionId'"
[[ "$(sql "select count(*) from event_stream where session_id='$ALLOW' and event_type='command.completed' and payload ? 'commandVersionId'")" -gt 0 ]]

printf '\n[idempotent init-db does not clobber]\n'
SEED_VERSION_BEFORE=$(sql "select current_version_id from command_definitions where action_id='cmd.rg.run'")
run cargo run --quiet -- init-db
SEED_VERSION_AFTER=$(sql "select current_version_id from command_definitions where action_id='cmd.rg.run'")
printf 'seed_current_version_before=%s after=%s\n' "$SEED_VERSION_BEFORE" "$SEED_VERSION_AFTER"
[[ "$SEED_VERSION_BEFORE" == "$SEED_VERSION_AFTER" ]]
run psql "$ROBDEX_AGENT_RUNTIME_DATABASE_URL" -c "UPDATE command_definitions SET enabled=false WHERE action_id='cmd.rg.run'"
run cargo run --quiet -- init-db
printf 'seed_disabled_after_init='; sql "select enabled from command_definitions where action_id='cmd.rg.run'"
[[ "$(sql "select enabled from command_definitions where action_id='cmd.rg.run'")" == "f" ]]
run psql "$ROBDEX_AGENT_RUNTIME_DATABASE_URL" -c "UPDATE command_definitions SET enabled=true WHERE action_id='cmd.rg.run'"
write_request /tmp/agent-runtime-seed-repoint.json update cmd.rg.run rg_repointed metadataOnlyProbe
SEED_REPOINT_REQ=$(cargo run --quiet -- command-registry requests create --session "$ADMIN_SESSION" /tmp/agent-runtime-seed-repoint.json)
run cargo run --quiet -- command-registry requests decide --session "$ADMIN_SESSION" "$SEED_REPOINT_REQ" --status approved
run cargo run --quiet -- command-registry requests apply --session "$ADMIN_SESSION" "$SEED_REPOINT_REQ"
REPOINTED_VERSION_BEFORE=$(sql "select current_version_id from command_definitions where action_id='cmd.rg.run'")
REPOINTED_OBJECT_BEFORE=$(sql "select cv.config->>'starlarkObject' from command_definitions cd join command_versions cv on cv.id=cd.current_version_id where cd.action_id='cmd.rg.run'")
run cargo run --quiet -- init-db
REPOINTED_VERSION_AFTER=$(sql "select current_version_id from command_definitions where action_id='cmd.rg.run'")
REPOINTED_OBJECT_AFTER=$(sql "select cv.config->>'starlarkObject' from command_definitions cd join command_versions cv on cv.id=cd.current_version_id where cd.action_id='cmd.rg.run'")
printf 'seed_repoint_before=%s/%s after=%s/%s\n' "$REPOINTED_VERSION_BEFORE" "$REPOINTED_OBJECT_BEFORE" "$REPOINTED_VERSION_AFTER" "$REPOINTED_OBJECT_AFTER"
[[ "$REPOINTED_VERSION_BEFORE" == "$REPOINTED_VERSION_AFTER" ]]
[[ "$REPOINTED_OBJECT_AFTER" == "rg_repointed" ]]
SEED_REFRESH_BEFORE=$(sql "select count(*) from command_registry_requests")
run cargo run --quiet -- command-registry seed-requests --session "$ADMIN_SESSION" --mode refresh
SEED_REFRESH_AFTER=$(sql "select count(*) from command_registry_requests")
printf 'seed_refresh_requests_before=%s after=%s\n' "$SEED_REFRESH_BEFORE" "$SEED_REFRESH_AFTER"
[[ "$SEED_REFRESH_AFTER" -gt "$SEED_REFRESH_BEFORE" ]]

write_request /tmp/agent-runtime-command-request.json add cmd.rg.files rg_files
REQ_ID=$(cargo run --quiet -- command-registry requests create --session "$ADMIN_SESSION" /tmp/agent-runtime-command-request.json)
printf 'request_id=%s\n' "$REQ_ID"
APPLY_BEFORE=$(cargo run --quiet -- command-registry requests apply --session "$ADMIN_SESSION" "$REQ_ID" 2>&1 || true)
printf 'apply_before_approval=%s\n' "$APPLY_BEFORE" | rg 'must be approved'
run cargo run --quiet -- command-registry requests decide --session "$ADMIN_SESSION" "$REQ_ID" --status denied
DENIED_APPLY=$(cargo run --quiet -- command-registry requests apply --session "$ADMIN_SESSION" "$REQ_ID" 2>&1 || true)
printf 'denied_apply=%s\n' "$DENIED_APPLY" | rg 'must be approved'
printf 'denied_registry_count='; sql "select count(*) from command_definitions where action_id='cmd.rg.files'"
[[ "$(sql "select count(*) from command_definitions where action_id='cmd.rg.files'")" -eq 0 ]]
REQ_ID2=$(cargo run --quiet -- command-registry requests create --session "$ADMIN_SESSION" /tmp/agent-runtime-command-request.json)
run cargo run --quiet -- command-registry requests decide --session "$ADMIN_SESSION" "$REQ_ID2" --status approved
run cargo run --quiet -- command-registry requests apply --session "$ADMIN_SESSION" "$REQ_ID2"
printf 'approved_registry_count='; sql "select count(*) from command_definitions where action_id='cmd.rg.files'"
[[ "$(sql "select count(*) from command_definitions where action_id='cmd.rg.files'")" -eq 1 ]]

DUP_REQ=$(cargo run --quiet -- command-registry requests create --session "$ADMIN_SESSION" /tmp/agent-runtime-command-request.json)
run cargo run --quiet -- command-registry requests decide --session "$ADMIN_SESSION" "$DUP_REQ" --status approved
DUP_APPLY=$(cargo run --quiet -- command-registry requests apply --session "$ADMIN_SESSION" "$DUP_REQ" 2>&1 || true)
printf 'duplicate_add_apply=%s\n' "$DUP_APPLY" | rg 'already exists'
printf 'duplicate_add_status='; sql "select application_status from command_registry_requests where id='$DUP_REQ'"
[[ "$(sql "select application_status from command_registry_requests where id='$DUP_REQ'")" == "pending" ]]

write_request /tmp/agent-runtime-missing-update.json update cmd.rg.missing rg_missing
MISSING_UPDATE=$(cargo run --quiet -- command-registry requests create --session "$ADMIN_SESSION" /tmp/agent-runtime-missing-update.json)
run cargo run --quiet -- command-registry requests decide --session "$ADMIN_SESSION" "$MISSING_UPDATE" --status approved
MISSING_UPDATE_APPLY=$(cargo run --quiet -- command-registry requests apply --session "$ADMIN_SESSION" "$MISSING_UPDATE" 2>&1 || true)
printf 'missing_update_apply=%s\n' "$MISSING_UPDATE_APPLY" | rg 'does not exist'
printf 'missing_update_status='; sql "select application_status from command_registry_requests where id='$MISSING_UPDATE'"
[[ "$(sql "select application_status from command_registry_requests where id='$MISSING_UPDATE'")" == "pending" ]]

write_request /tmp/agent-runtime-missing-enable.json enable cmd.rg.missing_enable rg_missing_enable
MISSING_ENABLE=$(cargo run --quiet -- command-registry requests create --session "$ADMIN_SESSION" /tmp/agent-runtime-missing-enable.json)
run cargo run --quiet -- command-registry requests decide --session "$ADMIN_SESSION" "$MISSING_ENABLE" --status approved
MISSING_ENABLE_APPLY=$(cargo run --quiet -- command-registry requests apply --session "$ADMIN_SESSION" "$MISSING_ENABLE" 2>&1 || true)
printf 'missing_enable_apply=%s\n' "$MISSING_ENABLE_APPLY" | rg 'did not change exactly one disabled row'
printf 'missing_enable_status='; sql "select application_status from command_registry_requests where id='$MISSING_ENABLE'"
[[ "$(sql "select application_status from command_registry_requests where id='$MISSING_ENABLE'")" == "pending" ]]

write_request /tmp/agent-runtime-missing-disable.json disable cmd.rg.missing_disable rg_missing_disable
MISSING_DISABLE=$(cargo run --quiet -- command-registry requests create --session "$ADMIN_SESSION" /tmp/agent-runtime-missing-disable.json)
run cargo run --quiet -- command-registry requests decide --session "$ADMIN_SESSION" "$MISSING_DISABLE" --status approved
MISSING_DISABLE_APPLY=$(cargo run --quiet -- command-registry requests apply --session "$ADMIN_SESSION" "$MISSING_DISABLE" 2>&1 || true)
printf 'missing_disable_apply=%s\n' "$MISSING_DISABLE_APPLY" | rg 'did not change exactly one enabled row'
printf 'missing_disable_status='; sql "select application_status from command_registry_requests where id='$MISSING_DISABLE'"
[[ "$(sql "select application_status from command_registry_requests where id='$MISSING_DISABLE'")" == "pending" ]]

write_request /tmp/agent-runtime-disable.json disable cmd.rg.files rg_files
DISABLE_REQ=$(cargo run --quiet -- command-registry requests create --session "$ADMIN_SESSION" /tmp/agent-runtime-disable.json)
run cargo run --quiet -- command-registry requests decide --session "$ADMIN_SESSION" "$DISABLE_REQ" --status approved
run cargo run --quiet -- command-registry requests apply --session "$ADMIN_SESSION" "$DISABLE_REQ"
printf 'disable_enabled_state='; sql "select enabled from command_definitions where action_id='cmd.rg.files'"
[[ "$(sql "select enabled from command_definitions where action_id='cmd.rg.files'")" == "f" ]]
DISABLE_AGAIN=$(cargo run --quiet -- command-registry requests create --session "$ADMIN_SESSION" /tmp/agent-runtime-disable.json)
run cargo run --quiet -- command-registry requests decide --session "$ADMIN_SESSION" "$DISABLE_AGAIN" --status approved
DISABLE_AGAIN_APPLY=$(cargo run --quiet -- command-registry requests apply --session "$ADMIN_SESSION" "$DISABLE_AGAIN" 2>&1 || true)
printf 'disable_again_apply=%s\n' "$DISABLE_AGAIN_APPLY" | rg 'did not change exactly one enabled row'
printf 'disable_again_status='; sql "select application_status from command_registry_requests where id='$DISABLE_AGAIN'"
[[ "$(sql "select application_status from command_registry_requests where id='$DISABLE_AGAIN'")" == "pending" ]]
write_request /tmp/agent-runtime-enable.json enable cmd.rg.files rg_files
ENABLE_REQ=$(cargo run --quiet -- command-registry requests create --session "$ADMIN_SESSION" /tmp/agent-runtime-enable.json)
run cargo run --quiet -- command-registry requests decide --session "$ADMIN_SESSION" "$ENABLE_REQ" --status approved
run cargo run --quiet -- command-registry requests apply --session "$ADMIN_SESSION" "$ENABLE_REQ"
printf 'enable_enabled_state='; sql "select enabled from command_definitions where action_id='cmd.rg.files'"
[[ "$(sql "select enabled from command_definitions where action_id='cmd.rg.files'")" == "t" ]]

write_request /tmp/agent-runtime-approval-apply.json add cmd.rg.applyapproval rg_applyapproval
REQ_ID3=$(cargo run --quiet -- command-registry requests create --session "$ADMIN_SESSION" /tmp/agent-runtime-approval-apply.json)
run cargo run --quiet -- command-registry requests decide --session "$ADMIN_SESSION" "$REQ_ID3" --status approved
python3 - <<'PY2'
import json
from pathlib import Path
d=json.load(open('roles/runtime-allow.json'))
d['prompt']['path']=str(Path('roles/prompts/runtime-allow.md').resolve())
d['policy']['command_registry.apply']='ownerApproval'
open('/tmp/runtime-registry-apply-approval.json','w').write(json.dumps(d))
PY2
run cargo run --quiet -- roles import /tmp/runtime-registry-apply-approval.json
APPLY_APPROVAL_SESSION=$(cargo run --quiet -- new-session --role runtime-allow)
APPLY_APPROVAL_OUT=$(cargo run --quiet -- command-registry requests apply --session "$APPLY_APPROVAL_SESSION" "$REQ_ID3" 2>&1 || true)
printf 'registry_apply_approval=%s\n' "$APPLY_APPROVAL_OUT" | rg 'requires approval'
printf 'registry_apply_approval_events='; sql "select count(*) from approval_requests where session_id='$APPLY_APPROVAL_SESSION' and action_name='command_registry.apply'"
[[ "$(sql "select count(*) from approval_requests where session_id='$APPLY_APPROVAL_SESSION' and action_name='command_registry.apply'")" -gt 0 ]]
printf 'registry_apply_not_applied='; sql "select application_status from command_registry_requests where id='$REQ_ID3'"
[[ "$(sql "select application_status from command_registry_requests where id='$REQ_ID3'")" == "pending" ]]
REGISTRY_APPLY_APPROVAL_ID=$(sql "select approval_request_id from command_registry_requests where id='$REQ_ID3'")
printf 'registry_apply_approval_id=%s\n' "$REGISTRY_APPLY_APPROVAL_ID"
run cargo run --quiet -- approvals decide "$REGISTRY_APPLY_APPROVAL_ID" --decision approved --reason "registry apply validation approval"
run cargo run --quiet -- command-registry requests apply --session "$APPLY_APPROVAL_SESSION" "$REQ_ID3"
printf 'registry_apply_after_approval='; sql "select application_status from command_registry_requests where id='$REQ_ID3'"
[[ "$(sql "select application_status from command_registry_requests where id='$REQ_ID3'")" == "applied" ]]

write_request /tmp/agent-runtime-metadata-class.json add cmd.rg.metadata rg_metadata metadataOnlyProbe
META_REQ=$(cargo run --quiet -- command-registry requests create --session "$ADMIN_SESSION" /tmp/agent-runtime-metadata-class.json)
run cargo run --quiet -- command-registry requests decide --session "$ADMIN_SESSION" "$META_REQ" --status approved
run cargo run --quiet -- command-registry requests apply --session "$ADMIN_SESSION" "$META_REQ"
printf 'mutation_class_stored='; sql "select cv.config->>'mutationClass' from command_definitions cd join command_versions cv on cv.id=cd.current_version_id where cd.action_id='cmd.rg.metadata'"
[[ "$(sql "select cv.config->>'mutationClass' from command_definitions cd join command_versions cv on cv.id=cd.current_version_id where cd.action_id='cmd.rg.metadata'")" == "metadataOnlyProbe" ]]

python3 - <<'PY'
import json
p='roles/runtime-allow.json'
d=json.load(open(p))
d['prompt']['path']=str(__import__('pathlib').Path('roles/prompts/runtime-allow.md').resolve())
for action in ['cmd.rg.files','cmd.rg.metadata']:
    if action not in d['capabilities']:
        d['capabilities'].append(action)
    d['policy'][action]='allow'
open('/tmp/runtime-allow-rg-files.json','w').write(json.dumps(d))
PY
run cargo run --quiet -- roles import /tmp/runtime-allow-rg-files.json
LIVE=$(cargo run --quiet -- new-session --role runtime-allow)
run cargo run --quiet -- --workdir . send --session "$LIVE" --message 'Use execute_code with exactly this Starlark source: files = cmd["rg_files"].run(args=["-g", "Cargo.toml"], cwd="."); output(files)'
printf 'live_new_command_runs='; sql "select count(*) from command_runs cr join command_versions cv on cv.id=cr.command_version_id where cv.action_id='cmd.rg.files'"
[[ "$(sql "select count(*) from command_runs cr join command_versions cv on cv.id=cr.command_version_id where cv.action_id='cmd.rg.files'")" -gt 0 ]]
run cargo run --quiet -- --workdir . send --session "$LIVE" --message 'Use execute_code with exactly this Starlark source: files = cmd["rg_metadata"].run(args=["-g", "Cargo.toml"], cwd="."); output(files)'
printf 'metadata_class_command_runs='; sql "select count(*) from command_runs cr join command_versions cv on cv.id=cr.command_version_id where cv.action_id='cmd.rg.metadata'"
[[ "$(sql "select count(*) from command_runs cr join command_versions cv on cv.id=cr.command_version_id where cv.action_id='cmd.rg.metadata'")" -gt 0 ]]

python3 - <<'PY'
import json
p='roles/runtime-allow.json'
d=json.load(open(p))
d['prompt']['path']=str(__import__('pathlib').Path('roles/prompts/runtime-allow.md').resolve())
if 'cmd.rg.files' not in d['capabilities']:
    d['capabilities'].append('cmd.rg.files')
d['policy']['cmd.rg.files']='deny'
open('/tmp/runtime-deny-rg-files.json','w').write(json.dumps(d))
d['policy']['cmd.rg.files']='ownerApproval'
open('/tmp/runtime-approval-rg-files.json','w').write(json.dumps(d))
PY
run cargo run --quiet -- roles import /tmp/runtime-deny-rg-files.json
DENY_SESSION=$(cargo run --quiet -- new-session --role runtime-allow)
run cargo run --quiet -- --workdir . send --session "$DENY_SESSION" --message 'Use execute_code with exactly this Starlark source: files = cmd["rg_files"].run(args=["-g", "Cargo.toml"], cwd="."); output(files)'
printf 'db_action_deny_events='; sql "select count(*) from event_stream where session_id='$DENY_SESSION' and event_type='policy.decision' and status='deny' and payload->>'action'='cmd.rg.files'"
[[ "$(sql "select count(*) from event_stream where session_id='$DENY_SESSION' and event_type='policy.decision' and status='deny' and payload->>'action'='cmd.rg.files'")" -gt 0 ]]
run cargo run --quiet -- roles import /tmp/runtime-approval-rg-files.json
APPROVAL_SESSION=$(cargo run --quiet -- new-session --role runtime-allow)
run cargo run --quiet -- --workdir . send --session "$APPROVAL_SESSION" --message 'Use execute_code with exactly this Starlark source: files = cmd["rg_files"].run(args=["-g", "Cargo.toml"], cwd="."); output(files)'
printf 'db_action_approval_events='; sql "select count(*) from event_stream where session_id='$APPROVAL_SESSION' and event_type='policy.decision' and status='approvalRequired' and payload->>'action'='cmd.rg.files'"
[[ "$(sql "select count(*) from event_stream where session_id='$APPROVAL_SESSION' and event_type='policy.decision' and status='approvalRequired' and payload->>'action'='cmd.rg.files'")" -gt 0 ]]
run cargo run --quiet -- command-registry show cmd.rg.files

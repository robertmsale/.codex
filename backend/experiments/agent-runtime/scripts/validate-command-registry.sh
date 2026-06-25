#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
source scripts/lib-validation-db.sh
validation_setup_database
run() { printf '\n$ %s\n' "$*"; "$@"; }
sql() { psql "$ROBDEX_AGENT_RUNTIME_DATABASE_URL" -At -c "$1"; }
approve_request() {
  local id="$1" file="$2"
  run cargo run --quiet --bin robdex-agent-runtime -- command-registry requests decide --session "$ADMIN_SESSION" "$id" --status approved --final-scope global --final-policy allow --final-command-file "$file"
}
create_internal_request() {
  local file="$1"
  python3 - "$ROBDEX_AGENT_RUNTIME_DATABASE_URL" "$ADMIN_SESSION" "$file" <<'PY'
import json, subprocess, sys, uuid
db, session, file = sys.argv[1:]
data = json.load(open(file))
request_id = str(uuid.uuid4())
command = data["command"]
def lit(value):
    return "'" + value.replace("'", "''") + "'"
sql = f"""
INSERT INTO command_registry_requests
(id, session_id, operation, proposed_command, requester_context, rationale, recommended_policy, requester, requested_by_role, approval_status, application_status)
VALUES (
{lit(request_id)}, {lit(session)}, {lit(data['operation'])}, {lit(json.dumps(command))}::jsonb,
'{{"validationOnly":true,"source":"validate-command-registry.sh"}}'::jsonb,
{lit(data.get('rationale', 'validation internal request'))},
{lit(data.get('recommendedPolicy', 'validation only'))},
'validation-internal-helper',
'{{}}'::jsonb,
'pending',
'pending'
)
"""
subprocess.check_call(["psql", db, "-v", "ON_ERROR_STOP=1", "-Atc", sql], stdout=subprocess.DEVNULL)
print(request_id)
PY
}
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
    "syncAllowed":True,"asyncAllowed":True,"maxRuntimeMs":5000,"endOfTurnBehavior":"terminate","stdinPolicy":"forbid","minAwaitMs":0,"maxAwaitMs":60000,"outputBufferBytes":64000,"terminateGraceMs":1000,
    "outputLimitBytes":12000,
    "mutationClass":mutation_class,
    "modelDescription":f"validation helper for {action}"
  }
}
open(file,'w').write(json.dumps(req))
PY
}

run cargo run --quiet --bin robdex-agent-runtime -- init-db
run cargo run --quiet --bin robdex-agent-runtime -- roles import-seeds
printf 'seed_role_concrete_cmd_policy_count='; sql "select count(*) from role_versions, jsonb_object_keys(policy) action where action like 'cmd.%'"
[[ "$(sql "select count(*) from role_versions, jsonb_object_keys(policy) action where action like 'cmd.%'")" -eq 0 ]]
python3 - <<'PY'
import json, pathlib
d={
  "id": "runtime-invalid-cmd-role",
  "version": "1.0.0",
  "displayName": "Runtime Invalid Command Role",
  "prompt": {"path": str(pathlib.Path("roles/prompts/runtime-allow.md").resolve())},
  "modelDefaults": {"model": "gpt-5.5", "reasoningEffort": "medium"},
  "capabilities": ["tool.execute_code", "cmd.rg.run"],
  "policy": {"tool.execute_code": "allow", "cmd.rg.run": "allow"},
  "routing": {"mode": "direct", "defaultRecipient": "owner", "allowedRecipients": ["owner"], "reservedActions": ["message.send", "message.route"]},
  "visibility": {"listed": True, "ownerVisible": True},
  "lifecycleAuthority": {"canSpawnAgents": False, "canArchiveAgents": False, "reservedActions": ["agent.spawn.<role>", "agent.archive"]},
}
pathlib.Path("/tmp/agent-runtime-invalid-cmd-role.json").write_text(json.dumps(d))
PY
INVALID_CMD_ROLE=$(cargo run --quiet --bin robdex-agent-runtime -- roles import /tmp/agent-runtime-invalid-cmd-role.json 2>&1 || true)
printf 'invalid_cmd_role=%s\n' "$INVALID_CMD_ROLE" | rg 'concrete command actions are not valid role policy entries'
ADMIN_SESSION=$(cargo run --quiet --bin robdex-agent-runtime -- sessions new --role runtime-allow)
printf 'admin_session=%s\n' "$ADMIN_SESSION"
printf '\n[seeded registry]\n'; cargo run --quiet --bin robdex-agent-runtime -- command-registry list | tee /tmp/agent-runtime-command-registry-list.json
sql "select action_id || ':' || (current_version_id is not null) from command_definitions order by action_id"
printf 'seed_version_trace_columns='; sql "select count(*) from command_versions where action_id in ('cmd.rg.run','cmd.git.status','cmd.git.diff','cmd.cargo.check')"
ALLOW=$(cargo run --quiet --bin robdex-agent-runtime -- sessions new --role runtime-allow)
printf 'allow_session=%s\n' "$ALLOW"
run cargo run --quiet --bin robdex-agent-runtime -- send --session "$ALLOW" --message 'Use execute_code with exactly this Starlark source: matches = cmd["rg"].run(args=["--files", "-g", "Cargo.toml"], cwd=".").sync(); print(matches)'
printf 'command_version_id_count='; sql "select count(*) from command_runs where command_version_id is not null"
[[ "$(sql "select count(*) from command_runs where command_version_id is not null")" -gt 0 ]]
printf 'command_event_versions='; sql "select count(*) from event_stream where session_id='$ALLOW' and event_type='command.completed' and payload ? 'commandVersionId'"
[[ "$(sql "select count(*) from event_stream where session_id='$ALLOW' and event_type='command.completed' and payload ? 'commandVersionId'")" -gt 0 ]]

printf '\n[idempotent init-db does not clobber]\n'
SEED_VERSION_BEFORE=$(sql "select current_version_id from command_definitions where action_id='cmd.rg.run'")
run cargo run --quiet --bin robdex-agent-runtime -- init-db
SEED_VERSION_AFTER=$(sql "select current_version_id from command_definitions where action_id='cmd.rg.run'")
printf 'seed_current_version_before=%s after=%s\n' "$SEED_VERSION_BEFORE" "$SEED_VERSION_AFTER"
[[ "$SEED_VERSION_BEFORE" == "$SEED_VERSION_AFTER" ]]
run psql "$ROBDEX_AGENT_RUNTIME_DATABASE_URL" -c "UPDATE command_definitions SET enabled=false WHERE action_id='cmd.rg.run'"
run cargo run --quiet --bin robdex-agent-runtime -- init-db
printf 'seed_disabled_after_init='; sql "select enabled from command_definitions where action_id='cmd.rg.run'"
[[ "$(sql "select enabled from command_definitions where action_id='cmd.rg.run'")" == "f" ]]
run psql "$ROBDEX_AGENT_RUNTIME_DATABASE_URL" -c "UPDATE command_definitions SET enabled=true WHERE action_id='cmd.rg.run'"
write_request /tmp/agent-runtime-seed-repoint.json update cmd.rg.run rg_repointed metadataOnlyProbe
SEED_REPOINT_REQ=$(create_internal_request /tmp/agent-runtime-seed-repoint.json)
approve_request "$SEED_REPOINT_REQ" /tmp/agent-runtime-seed-repoint.json
run cargo run --quiet --bin robdex-agent-runtime -- command-registry requests apply --session "$ADMIN_SESSION" "$SEED_REPOINT_REQ"
REPOINTED_VERSION_BEFORE=$(sql "select current_version_id from command_definitions where action_id='cmd.rg.run'")
REPOINTED_OBJECT_BEFORE=$(sql "select cv.config->>'starlarkObject' from command_definitions cd join command_versions cv on cv.id=cd.current_version_id where cd.action_id='cmd.rg.run'")
run cargo run --quiet --bin robdex-agent-runtime -- init-db
REPOINTED_VERSION_AFTER=$(sql "select current_version_id from command_definitions where action_id='cmd.rg.run'")
REPOINTED_OBJECT_AFTER=$(sql "select cv.config->>'starlarkObject' from command_definitions cd join command_versions cv on cv.id=cd.current_version_id where cd.action_id='cmd.rg.run'")
printf 'seed_repoint_before=%s/%s after=%s/%s\n' "$REPOINTED_VERSION_BEFORE" "$REPOINTED_OBJECT_BEFORE" "$REPOINTED_VERSION_AFTER" "$REPOINTED_OBJECT_AFTER"
[[ "$REPOINTED_VERSION_BEFORE" == "$REPOINTED_VERSION_AFTER" ]]
[[ "$REPOINTED_OBJECT_AFTER" == "rg_repointed" ]]
SEED_REFRESH_BEFORE=$(sql "select count(*) from command_registry_requests")
run cargo run --quiet --bin robdex-agent-runtime -- command-registry seed-requests --session "$ADMIN_SESSION" --mode refresh
SEED_REFRESH_AFTER=$(sql "select count(*) from command_registry_requests")
printf 'seed_refresh_requests_before=%s after=%s\n' "$SEED_REFRESH_BEFORE" "$SEED_REFRESH_AFTER"
[[ "$SEED_REFRESH_AFTER" -gt "$SEED_REFRESH_BEFORE" ]]

write_request /tmp/agent-runtime-command-request.json add cmd.rg.files rg_files
REQ_ID=$(create_internal_request /tmp/agent-runtime-command-request.json)
printf 'request_id=%s\n' "$REQ_ID"
APPLY_BEFORE=$(cargo run --quiet --bin robdex-agent-runtime -- command-registry requests apply --session "$ADMIN_SESSION" "$REQ_ID" 2>&1 || true)
printf 'apply_before_approval=%s\n' "$APPLY_BEFORE" | rg 'must be approved'
run cargo run --quiet --bin robdex-agent-runtime -- command-registry requests decide --session "$ADMIN_SESSION" "$REQ_ID" --status denied
DENIED_APPLY=$(cargo run --quiet --bin robdex-agent-runtime -- command-registry requests apply --session "$ADMIN_SESSION" "$REQ_ID" 2>&1 || true)
printf 'denied_apply=%s\n' "$DENIED_APPLY" | rg 'must be approved'
printf 'denied_registry_count='; sql "select count(*) from command_definitions where action_id='cmd.rg.files'"
[[ "$(sql "select count(*) from command_definitions where action_id='cmd.rg.files'")" -eq 0 ]]
REQ_ID2=$(create_internal_request /tmp/agent-runtime-command-request.json)
MISSING_FINAL_DECIDE=$(cargo run --quiet --bin robdex-agent-runtime -- command-registry requests decide --session "$ADMIN_SESSION" "$REQ_ID2" --status approved 2>&1 || true)
printf 'missing_final_decide=%s\n' "$MISSING_FINAL_DECIDE" | rg 'requires final scope'
printf 'missing_final_decide_status='; sql "select approval_status || '/' || application_status from command_registry_requests where id='$REQ_ID2'"
[[ "$(sql "select approval_status || '/' || application_status from command_registry_requests where id='$REQ_ID2'")" == "pending/pending" ]]
approve_request "$REQ_ID2" /tmp/agent-runtime-command-request.json
run cargo run --quiet --bin robdex-agent-runtime -- command-registry requests apply --session "$ADMIN_SESSION" "$REQ_ID2"
printf 'approved_registry_count='; sql "select count(*) from command_definitions where action_id='cmd.rg.files'"
[[ "$(sql "select count(*) from command_definitions where action_id='cmd.rg.files'")" -eq 1 ]]

DUP_REQ=$(create_internal_request /tmp/agent-runtime-command-request.json)
DUP_DECIDE=$(cargo run --quiet --bin robdex-agent-runtime -- command-registry requests decide --session "$ADMIN_SESSION" "$DUP_REQ" --status approved --final-scope global --final-policy allow --final-command-file /tmp/agent-runtime-command-request.json 2>&1 || true)
printf 'duplicate_add_decide=%s\n' "$DUP_DECIDE" | rg 'already exists'
printf 'duplicate_add_status='; sql "select approval_status || '/' || application_status from command_registry_requests where id='$DUP_REQ'"
[[ "$(sql "select approval_status || '/' || application_status from command_registry_requests where id='$DUP_REQ'")" == "pending/pending" ]]

write_request /tmp/agent-runtime-missing-update.json update cmd.rg.missing rg_missing
MISSING_UPDATE=$(create_internal_request /tmp/agent-runtime-missing-update.json)
MISSING_UPDATE_DECIDE=$(cargo run --quiet --bin robdex-agent-runtime -- command-registry requests decide --session "$ADMIN_SESSION" "$MISSING_UPDATE" --status approved --final-scope global --final-policy allow --final-command-file /tmp/agent-runtime-missing-update.json 2>&1 || true)
printf 'missing_update_decide=%s\n' "$MISSING_UPDATE_DECIDE" | rg 'does not exist'
printf 'missing_update_status='; sql "select approval_status || '/' || application_status from command_registry_requests where id='$MISSING_UPDATE'"
[[ "$(sql "select approval_status || '/' || application_status from command_registry_requests where id='$MISSING_UPDATE'")" == "pending/pending" ]]

write_request /tmp/agent-runtime-missing-enable.json enable cmd.rg.missing_enable rg_missing_enable
MISSING_ENABLE=$(create_internal_request /tmp/agent-runtime-missing-enable.json)
MISSING_ENABLE_DECIDE=$(cargo run --quiet --bin robdex-agent-runtime -- command-registry requests decide --session "$ADMIN_SESSION" "$MISSING_ENABLE" --status approved --final-scope global --final-policy allow --final-command-file /tmp/agent-runtime-missing-enable.json 2>&1 || true)
printf 'missing_enable_decide=%s\n' "$MISSING_ENABLE_DECIDE" | rg 'does not exist|did not change exactly one disabled row'
printf 'missing_enable_status='; sql "select approval_status || '/' || application_status from command_registry_requests where id='$MISSING_ENABLE'"
[[ "$(sql "select approval_status || '/' || application_status from command_registry_requests where id='$MISSING_ENABLE'")" == "pending/pending" ]]

write_request /tmp/agent-runtime-missing-disable.json disable cmd.rg.missing_disable rg_missing_disable
MISSING_DISABLE=$(create_internal_request /tmp/agent-runtime-missing-disable.json)
MISSING_DISABLE_DECIDE=$(cargo run --quiet --bin robdex-agent-runtime -- command-registry requests decide --session "$ADMIN_SESSION" "$MISSING_DISABLE" --status approved --final-scope global --final-policy allow --final-command-file /tmp/agent-runtime-missing-disable.json 2>&1 || true)
printf 'missing_disable_decide=%s\n' "$MISSING_DISABLE_DECIDE" | rg 'does not exist|did not change exactly one enabled row'
printf 'missing_disable_status='; sql "select approval_status || '/' || application_status from command_registry_requests where id='$MISSING_DISABLE'"
[[ "$(sql "select approval_status || '/' || application_status from command_registry_requests where id='$MISSING_DISABLE'")" == "pending/pending" ]]

write_request /tmp/agent-runtime-disable.json disable cmd.rg.files rg_files
DISABLE_REQ=$(create_internal_request /tmp/agent-runtime-disable.json)
approve_request "$DISABLE_REQ" /tmp/agent-runtime-disable.json
run cargo run --quiet --bin robdex-agent-runtime -- command-registry requests apply --session "$ADMIN_SESSION" "$DISABLE_REQ"
printf 'disable_enabled_state='; sql "select enabled from command_definitions where action_id='cmd.rg.files'"
[[ "$(sql "select enabled from command_definitions where action_id='cmd.rg.files'")" == "f" ]]
DISABLE_AGAIN=$(create_internal_request /tmp/agent-runtime-disable.json)
DISABLE_AGAIN_DECIDE=$(cargo run --quiet --bin robdex-agent-runtime -- command-registry requests decide --session "$ADMIN_SESSION" "$DISABLE_AGAIN" --status approved --final-scope global --final-policy allow --final-command-file /tmp/agent-runtime-disable.json 2>&1 || true)
printf 'disable_again_decide=%s\n' "$DISABLE_AGAIN_DECIDE" | rg 'did not change exactly one enabled row'
printf 'disable_again_status='; sql "select approval_status || '/' || application_status from command_registry_requests where id='$DISABLE_AGAIN'"
[[ "$(sql "select approval_status || '/' || application_status from command_registry_requests where id='$DISABLE_AGAIN'")" == "pending/pending" ]]
write_request /tmp/agent-runtime-enable.json enable cmd.rg.files rg_files
ENABLE_REQ=$(create_internal_request /tmp/agent-runtime-enable.json)
approve_request "$ENABLE_REQ" /tmp/agent-runtime-enable.json
run cargo run --quiet --bin robdex-agent-runtime -- command-registry requests apply --session "$ADMIN_SESSION" "$ENABLE_REQ"
printf 'enable_enabled_state='; sql "select enabled from command_definitions where action_id='cmd.rg.files'"
[[ "$(sql "select enabled from command_definitions where action_id='cmd.rg.files'")" == "t" ]]
ENABLE_ALREADY_VERSION_BEFORE=$(sql "select current_version_id from command_definitions where action_id='cmd.rg.files'")
ENABLE_ALREADY_VERSION_COUNT_BEFORE=$(sql "select count(*) from command_versions cv join command_definitions cd on cd.id=cv.definition_id where cd.action_id='cmd.rg.files'")
write_request /tmp/agent-runtime-enable-again.json enable cmd.rg.files rg_files_enable_again
ENABLE_AGAIN=$(create_internal_request /tmp/agent-runtime-enable-again.json)
ENABLE_AGAIN_DECIDE=$(cargo run --quiet --bin robdex-agent-runtime -- command-registry requests decide --session "$ADMIN_SESSION" "$ENABLE_AGAIN" --status approved --final-scope global --final-policy allow --final-command-file /tmp/agent-runtime-enable-again.json 2>&1 || true)
printf 'enable_again_decide=%s\n' "$ENABLE_AGAIN_DECIDE" | rg 'did not change exactly one disabled row'
printf 'enable_again_status='; sql "select approval_status || '/' || application_status from command_registry_requests where id='$ENABLE_AGAIN'"
[[ "$(sql "select approval_status || '/' || application_status from command_registry_requests where id='$ENABLE_AGAIN'")" == "pending/pending" ]]
ENABLE_ALREADY_VERSION_AFTER=$(sql "select current_version_id from command_definitions where action_id='cmd.rg.files'")
ENABLE_ALREADY_VERSION_COUNT_AFTER=$(sql "select count(*) from command_versions cv join command_definitions cd on cd.id=cv.definition_id where cd.action_id='cmd.rg.files'")
printf 'enable_again_version_before=%s after=%s\n' "$ENABLE_ALREADY_VERSION_BEFORE" "$ENABLE_ALREADY_VERSION_AFTER"
[[ "$ENABLE_ALREADY_VERSION_BEFORE" == "$ENABLE_ALREADY_VERSION_AFTER" ]]
printf 'enable_again_version_count_before=%s after=%s\n' "$ENABLE_ALREADY_VERSION_COUNT_BEFORE" "$ENABLE_ALREADY_VERSION_COUNT_AFTER"
[[ "$ENABLE_ALREADY_VERSION_COUNT_BEFORE" == "$ENABLE_ALREADY_VERSION_COUNT_AFTER" ]]

write_request /tmp/agent-runtime-approval-apply.json add cmd.rg.applyapproval rg_applyapproval
REQ_ID3=$(create_internal_request /tmp/agent-runtime-approval-apply.json)
approve_request "$REQ_ID3" /tmp/agent-runtime-approval-apply.json
python3 - <<'PY2'
import json
from pathlib import Path
d=json.load(open('roles/runtime-allow.json'))
d['prompt']['path']=str(Path('roles/prompts/runtime-allow.md').resolve())
d['policy']['command_registry.apply']='ownerApproval'
open('/tmp/runtime-registry-apply-approval.json','w').write(json.dumps(d))
PY2
run cargo run --quiet --bin robdex-agent-runtime -- roles import /tmp/runtime-registry-apply-approval.json
APPLY_APPROVAL_SESSION=$(cargo run --quiet --bin robdex-agent-runtime -- sessions new --role runtime-allow)
APPLY_APPROVAL_OUT=$(cargo run --quiet --bin robdex-agent-runtime -- command-registry requests apply --session "$APPLY_APPROVAL_SESSION" "$REQ_ID3" 2>&1 || true)
printf 'registry_apply_approval=%s\n' "$APPLY_APPROVAL_OUT" | rg 'requires approval'
printf 'registry_apply_approval_events='; sql "select count(*) from approval_requests where session_id='$APPLY_APPROVAL_SESSION' and action_name='command_registry.apply'"
[[ "$(sql "select count(*) from approval_requests where session_id='$APPLY_APPROVAL_SESSION' and action_name='command_registry.apply'")" -gt 0 ]]
printf 'registry_apply_not_applied='; sql "select application_status from command_registry_requests where id='$REQ_ID3'"
[[ "$(sql "select application_status from command_registry_requests where id='$REQ_ID3'")" == "pending" ]]
REGISTRY_APPLY_APPROVAL_ID=$(sql "select approval_request_id from command_registry_requests where id='$REQ_ID3'")
printf 'registry_apply_approval_id=%s\n' "$REGISTRY_APPLY_APPROVAL_ID"
run cargo run --quiet --bin robdex-agent-runtime -- approvals decide "$REGISTRY_APPLY_APPROVAL_ID" --decision approved --reason "registry apply validation approval"
run cargo run --quiet --bin robdex-agent-runtime -- command-registry requests apply --session "$APPLY_APPROVAL_SESSION" "$REQ_ID3"
printf 'registry_apply_after_approval='; sql "select application_status from command_registry_requests where id='$REQ_ID3'"
[[ "$(sql "select application_status from command_registry_requests where id='$REQ_ID3'")" == "applied" ]]

write_request /tmp/agent-runtime-metadata-class.json add cmd.rg.metadata rg_metadata metadataOnlyProbe
META_REQ=$(create_internal_request /tmp/agent-runtime-metadata-class.json)
approve_request "$META_REQ" /tmp/agent-runtime-metadata-class.json
run cargo run --quiet --bin robdex-agent-runtime -- command-registry requests apply --session "$ADMIN_SESSION" "$META_REQ"
printf 'mutation_class_stored='; sql "select cv.config->>'mutationClass' from command_definitions cd join command_versions cv on cv.id=cd.current_version_id where cd.action_id='cmd.rg.metadata'"
[[ "$(sql "select cv.config->>'mutationClass' from command_definitions cd join command_versions cv on cv.id=cd.current_version_id where cd.action_id='cmd.rg.metadata'")" == "metadataOnlyProbe" ]]

LIVE=$(cargo run --quiet --bin robdex-agent-runtime -- sessions new --role runtime-allow)
run cargo run --quiet --bin robdex-agent-runtime -- send --session "$LIVE" --message 'Use execute_code with exactly this Starlark source: files = cmd["rg_files"].run(args=["-g", "Cargo.toml"], cwd=".").sync(); print(files)'
printf 'live_new_command_runs='; sql "select count(*) from command_runs cr join command_versions cv on cv.id=cr.command_version_id where cv.action_id='cmd.rg.files'"
[[ "$(sql "select count(*) from command_runs cr join command_versions cv on cv.id=cr.command_version_id where cv.action_id='cmd.rg.files'")" -gt 0 ]]
run cargo run --quiet --bin robdex-agent-runtime -- send --session "$LIVE" --message 'Use execute_code with exactly this Starlark source: files = cmd["rg_metadata"].run(args=["-g", "Cargo.toml"], cwd=".").sync(); print(files)'
printf 'metadata_class_command_runs='; sql "select count(*) from command_runs cr join command_versions cv on cv.id=cr.command_version_id where cv.action_id='cmd.rg.metadata'"
[[ "$(sql "select count(*) from command_runs cr join command_versions cv on cv.id=cr.command_version_id where cv.action_id='cmd.rg.metadata'")" -gt 0 ]]

run cargo run --quiet --bin robdex-agent-runtime -- command-registry show cmd.rg.files

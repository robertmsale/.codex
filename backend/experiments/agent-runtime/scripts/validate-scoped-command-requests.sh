#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
source scripts/lib-validation-db.sh
validation_setup_database
run() { printf '\n$ %s\n' "$*"; "$@"; }
sql() { psql "$ROBDEX_AGENT_RUNTIME_DATABASE_URL" -At -c "$1"; }
write_seed() {
  local file="$1" action="$2" object="$3"
  python3 - "$file" "$action" "$object" <<'PY'
import json, sys
file, action, obj = sys.argv[1:]
seed={"actionId":action,"binaryName":"rg","candidatePaths":["/opt/homebrew/bin/rg","/usr/local/bin/rg","/usr/bin/rg"],"starlarkObject":obj,"starlarkMethod":"run","argvPrefix":["--files"],"allowArgsArg":True,"allowCwdArg":True,"defaultCwd":".","cwdPolicy":"underExecutionRoot","envPolicy":"empty","syncAllowed":True,"asyncAllowed":True,"maxRuntimeMs":5000,"endOfTurnBehavior":"terminate","stdinPolicy":"forbid","minAwaitMs":0,"maxAwaitMs":60000,"outputBufferBytes":64000,"terminateGraceMs":1000,"outputLimitBytes":12000,"mutationClass":"readOnly","modelDescription":f"scoped validation helper {action}","forbiddenArgs":[]}
open(file,'w').write(json.dumps(seed))
PY
}
write_request() {
  local file="$1" operation="$2" seed_file="$3"
  python3 - "$file" "$operation" "$seed_file" <<'PY'
import json, sys
file, operation, seed_file = sys.argv[1:]
seed=json.load(open(seed_file))
req={"operation":operation,"rationale":"scoped validation request","recommendedPolicy":"advisory only","requester":"validation-script","command":seed}
open(file,'w').write(json.dumps(req))
PY
}
approve() {
  local id="$1" scope="$2" project="$3" policy="$4" seed_file="$5"
  if [[ "$scope" == "project" ]]; then
    run cargo run --quiet --bin robdex-agent-runtime -- command-registry requests decide --session "$ADMIN" "$id" --status approved --final-scope project --final-project "$project" --final-policy "$policy" --final-command-file "$seed_file"
  else
    run cargo run --quiet --bin robdex-agent-runtime -- command-registry requests decide --session "$ADMIN" "$id" --status approved --final-scope global --final-policy "$policy" --final-command-file "$seed_file"
  fi
}
create_internal_request() {
  local file="$1"
  python3 - "$ROBDEX_AGENT_RUNTIME_DATABASE_URL" "$ADMIN" "$file" <<'PY'
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
'{{"validationOnly":true,"source":"validate-scoped-command-requests.sh"}}'::jsonb,
{lit(data.get('rationale', 'validation internal scoped request'))},
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

run cargo run --quiet --bin robdex-agent-runtime -- init-db
run cargo run --quiet --bin robdex-agent-runtime -- roles import-seeds
ADMIN=$(cargo run --quiet --bin robdex-agent-runtime -- sessions new --role runtime-allow --project admin)
printf 'admin_session=%s\n' "$ADMIN"

write_seed /tmp/scoped-native-seed.json cmd.rg.native_request rg_native_request
NATIVE_SESSION=$(cargo run --quiet --bin robdex-agent-runtime -- sessions new --role runtime-allow --project alpha)
run cargo run --quiet --bin robdex-agent-runtime -- send --session "$NATIVE_SESSION" --message 'Call request_command_registry_change exactly once. Use operation add. proposedCommand actionId cmd.rg.native_request, binaryName rg, candidatePaths ["/opt/homebrew/bin/rg","/usr/local/bin/rg","/usr/bin/rg"], starlarkObject rg_native_request, starlarkMethod run, argvPrefix ["--files"], defaultCwd ".", cwdPolicy "underExecutionRoot", envPolicy "empty", syncAllowed true, asyncAllowed true, maxRuntimeMs 5000, endOfTurnBehavior "terminate", stdinPolicy "forbid", minAwaitMs 0, maxAwaitMs 60000, outputBufferBytes 64000, terminateGraceMs 1000, outputLimitBytes 12000, mutationClass "readOnly", modelDescription "native request validation helper", allowCwdArg true, allowArgsArg true, forbiddenArgs []. rationale "need a native registry request validation command". intendedUse "validate native request tool". currentBlockerOrNeed "missing command surface". requesterContext sourceRole "runtime-allow", sourceTask "validation", observedError "missing command", neededFor "scoped request validation".'
printf 'native_request_count='; sql "select count(*) from command_registry_requests where requester='native-model-tool' and proposed_command->>'actionId'='cmd.rg.native_request' and final_scope is null and final_execution_policy is null"
[[ "$(sql "select count(*) from command_registry_requests where requester='native-model-tool' and proposed_command->>'actionId'='cmd.rg.native_request' and final_scope is null and final_execution_policy is null")" -ge 1 ]]
NATIVE_REQ=$(sql "select id from command_registry_requests where requester='native-model-tool' and proposed_command->>'actionId'='cmd.rg.native_request' order by created_at desc limit 1")
run cargo run --quiet --bin robdex-agent-runtime -- command-registry requests decide --session "$ADMIN" "$NATIVE_REQ" --status denied
printf 'native_denied_no_definition='; sql "select count(*) from command_definitions where action_id='cmd.rg.native_request'"
[[ "$(sql "select count(*) from command_definitions where action_id='cmd.rg.native_request'")" -eq 0 ]]
printf 'native_denied_status='; sql "select approval_status || '/' || application_status from command_registry_requests where id='$NATIVE_REQ'"
[[ "$(sql "select approval_status || '/' || application_status from command_registry_requests where id='$NATIVE_REQ'")" == "denied/pending" ]]

write_seed /tmp/scoped-global-seed.json cmd.rg.global_visible rg_global_visible
write_request /tmp/scoped-global-request.json add /tmp/scoped-global-seed.json
GLOBAL_REQ=$(create_internal_request /tmp/scoped-global-request.json)
MISSING_FINAL=$(cargo run --quiet --bin robdex-agent-runtime -- command-registry requests decide --session "$ADMIN" "$GLOBAL_REQ" --status approved 2>&1 || true)
printf 'scoped_missing_final=%s\n' "$MISSING_FINAL" | rg 'requires final scope'
approve "$GLOBAL_REQ" global '' allow /tmp/scoped-global-seed.json
run cargo run --quiet --bin robdex-agent-runtime -- command-registry requests apply --session "$ADMIN" "$GLOBAL_REQ"
GLOBAL_SESSION=$(cargo run --quiet --bin robdex-agent-runtime -- sessions new --role runtime-allow --project beta)
run cargo run --quiet --bin robdex-agent-runtime -- send --session "$GLOBAL_SESSION" --message 'Use execute_code with exactly this Starlark source: files = cmd["rg_global_visible"].run(args=["-g", "Cargo.toml"], cwd=".").sync(); print(files)'
printf 'global_visible_runs='; sql "select count(*) from command_runs cr join command_versions cv on cv.id=cr.command_version_id where cv.action_id='cmd.rg.global_visible'"
[[ "$(sql "select count(*) from command_runs cr join command_versions cv on cv.id=cr.command_version_id where cv.action_id='cmd.rg.global_visible'")" -gt 0 ]]

write_seed /tmp/scoped-global-deny-seed.json cmd.rg.global_denied rg_global_denied
write_request /tmp/scoped-global-deny-request.json add /tmp/scoped-global-deny-seed.json
GLOBAL_DENY_REQ=$(create_internal_request /tmp/scoped-global-deny-request.json)
approve "$GLOBAL_DENY_REQ" global '' deny /tmp/scoped-global-deny-seed.json
run cargo run --quiet --bin robdex-agent-runtime -- command-registry requests apply --session "$ADMIN" "$GLOBAL_DENY_REQ"
GLOBAL_DENY_SESSION=$(cargo run --quiet --bin robdex-agent-runtime -- sessions new --role runtime-allow --project beta)
GLOBAL_DENY_OUT=$(cargo run --quiet --bin robdex-agent-runtime -- send --session "$GLOBAL_DENY_SESSION" --message 'Use execute_code with exactly this Starlark source: files = cmd["rg_global_denied"].run(args=["-g", "Cargo.toml"], cwd=".").sync(); print(files)' 2>&1 || true)
printf 'global_deny_attempt=%s\n' "$GLOBAL_DENY_OUT"
printf 'global_deny_visible_context='; sql "select count(*) from event_stream where session_id='$GLOBAL_DENY_SESSION' and event_type='model.tool_call' and payload->'request'->'commandContext'->'summaries' @> '[{\"actionId\":\"cmd.rg.global_denied\",\"starlarkObject\":\"rg_global_denied\"}]'::jsonb"
[[ "$(sql "select count(*) from event_stream where session_id='$GLOBAL_DENY_SESSION' and event_type='model.tool_call' and payload->'request'->'commandContext'->'summaries' @> '[{\"actionId\":\"cmd.rg.global_denied\",\"starlarkObject\":\"rg_global_denied\"}]'::jsonb")" -gt 0 ]]
printf 'global_deny_stable_contract_mentions='; sql "select count(*) from event_stream where session_id='$GLOBAL_DENY_SESSION' and event_type='model.tool_call' and (payload->'request'->>'executeCodeContract' like '%rg_global_denied%' or payload->'request'->>'executeCodeContract' like '%cmd.rg.global_denied%')"
[[ "$(sql "select count(*) from event_stream where session_id='$GLOBAL_DENY_SESSION' and event_type='model.tool_call' and (payload->'request'->>'executeCodeContract' like '%rg_global_denied%' or payload->'request'->>'executeCodeContract' like '%cmd.rg.global_denied%')")" -eq 0 ]]
printf 'global_deny_policy_events='; sql "select count(*) from event_stream where session_id='$GLOBAL_DENY_SESSION' and event_type='policy.decision' and status='deny' and payload->>'action'='cmd.rg.global_denied'"
[[ "$(sql "select count(*) from event_stream where session_id='$GLOBAL_DENY_SESSION' and event_type='policy.decision' and status='deny' and payload->>'action'='cmd.rg.global_denied'")" -gt 0 ]]
printf 'global_deny_command_runs='; sql "select count(*) from command_runs cr join command_versions cv on cv.id=cr.command_version_id where cv.action_id='cmd.rg.global_denied'"
[[ "$(sql "select count(*) from command_runs cr join command_versions cv on cv.id=cr.command_version_id where cv.action_id='cmd.rg.global_denied'")" -eq 0 ]]

write_seed /tmp/scoped-global-orch-seed.json cmd.rg.global_orch_approval rg_global_orch_approval
write_request /tmp/scoped-global-orch-request.json add /tmp/scoped-global-orch-seed.json
GLOBAL_ORCH_REQ=$(create_internal_request /tmp/scoped-global-orch-request.json)
approve "$GLOBAL_ORCH_REQ" global '' orchestratorApproval /tmp/scoped-global-orch-seed.json
run cargo run --quiet --bin robdex-agent-runtime -- command-registry requests apply --session "$ADMIN" "$GLOBAL_ORCH_REQ"
GLOBAL_ORCH_SESSION=$(cargo run --quiet --bin robdex-agent-runtime -- sessions new --role runtime-allow --project beta)
GLOBAL_ORCH_OUT=$(cargo run --quiet --bin robdex-agent-runtime -- send --session "$GLOBAL_ORCH_SESSION" --message 'Use execute_code with exactly this Starlark source: files = cmd["rg_global_orch_approval"].run(args=["-g", "Cargo.toml"], cwd=".").sync(); print(files)' 2>&1 || true)
printf 'global_orch_approval_attempt=%s\n' "$GLOBAL_ORCH_OUT"
printf 'global_orch_approval_requests='; sql "select count(*) from approval_requests where session_id='$GLOBAL_ORCH_SESSION' and action_name='cmd.rg.global_orch_approval' and required_approver_kind='orchestrator'"
[[ "$(sql "select count(*) from approval_requests where session_id='$GLOBAL_ORCH_SESSION' and action_name='cmd.rg.global_orch_approval' and required_approver_kind='orchestrator'")" -gt 0 ]]
printf 'global_orch_command_runs='; sql "select count(*) from command_runs cr join command_versions cv on cv.id=cr.command_version_id where cv.action_id='cmd.rg.global_orch_approval'"
[[ "$(sql "select count(*) from command_runs cr join command_versions cv on cv.id=cr.command_version_id where cv.action_id='cmd.rg.global_orch_approval'")" -eq 0 ]]

write_seed /tmp/scoped-project-seed.json cmd.rg.project_visible rg_project_visible
write_request /tmp/scoped-project-request.json add /tmp/scoped-project-seed.json
PROJECT_REQ=$(create_internal_request /tmp/scoped-project-request.json)
approve "$PROJECT_REQ" project alpha allow /tmp/scoped-project-seed.json
run cargo run --quiet --bin robdex-agent-runtime -- command-registry requests apply --session "$ADMIN" "$PROJECT_REQ"
ALPHA_SESSION=$(cargo run --quiet --bin robdex-agent-runtime -- sessions new --role runtime-allow --project alpha)
BETA_SESSION=$(cargo run --quiet --bin robdex-agent-runtime -- sessions new --role runtime-allow --project beta)
run cargo run --quiet --bin robdex-agent-runtime -- send --session "$ALPHA_SESSION" --message 'Use execute_code with exactly this Starlark source: files = cmd["rg_project_visible"].run(args=["-g", "Cargo.toml"], cwd=".").sync(); print(files)'
BETA_OUT=$(cargo run --quiet --bin robdex-agent-runtime -- send --session "$BETA_SESSION" --message 'Use execute_code with exactly this Starlark source: files = cmd["rg_project_visible"].run(args=["-g", "Cargo.toml"], cwd=".").sync(); print(files)' 2>&1 || true)
printf 'beta_project_attempt=%s\n' "$BETA_OUT"
printf 'project_alpha_runs='; sql "select count(*) from command_runs cr join command_versions cv on cv.id=cr.command_version_id where cv.action_id='cmd.rg.project_visible' and cr.id in (select entity_id from event_stream where session_id='$ALPHA_SESSION' and event_type='command.completed')"
[[ "$(sql "select count(*) from command_runs cr join command_versions cv on cv.id=cr.command_version_id where cv.action_id='cmd.rg.project_visible' and cr.id in (select entity_id from event_stream where session_id='$ALPHA_SESSION' and event_type='command.completed')")" -gt 0 ]]
printf 'project_beta_runs='; sql "select count(*) from command_runs cr join command_versions cv on cv.id=cr.command_version_id where cv.action_id='cmd.rg.project_visible' and cr.id in (select entity_id from event_stream where session_id='$BETA_SESSION' and event_type='command.completed')"
[[ "$(sql "select count(*) from command_runs cr join command_versions cv on cv.id=cr.command_version_id where cv.action_id='cmd.rg.project_visible' and cr.id in (select entity_id from event_stream where session_id='$BETA_SESSION' and event_type='command.completed')")" -eq 0 ]]

write_seed /tmp/scoped-project-owner-seed.json cmd.rg.project_owner_approval rg_project_owner_approval
write_request /tmp/scoped-project-owner-request.json add /tmp/scoped-project-owner-seed.json
PROJECT_OWNER_REQ=$(create_internal_request /tmp/scoped-project-owner-request.json)
approve "$PROJECT_OWNER_REQ" project alpha ownerApproval /tmp/scoped-project-owner-seed.json
run cargo run --quiet --bin robdex-agent-runtime -- command-registry requests apply --session "$ADMIN" "$PROJECT_OWNER_REQ"
PROJECT_OWNER_SESSION=$(cargo run --quiet --bin robdex-agent-runtime -- sessions new --role runtime-allow --project alpha)
PROJECT_OWNER_OUT=$(cargo run --quiet --bin robdex-agent-runtime -- send --session "$PROJECT_OWNER_SESSION" --message 'Use execute_code with exactly this Starlark source: files = cmd["rg_project_owner_approval"].run(args=["-g", "Cargo.toml"], cwd=".").sync(); print(files)' 2>&1 || true)
printf 'project_owner_approval_attempt=%s\n' "$PROJECT_OWNER_OUT"
printf 'project_owner_approval_requests='; sql "select count(*) from approval_requests where session_id='$PROJECT_OWNER_SESSION' and action_name='cmd.rg.project_owner_approval' and required_approver_kind='owner'"
[[ "$(sql "select count(*) from approval_requests where session_id='$PROJECT_OWNER_SESSION' and action_name='cmd.rg.project_owner_approval' and required_approver_kind='owner'")" -gt 0 ]]
printf 'project_owner_paused_actions='; sql "select count(*) from paused_actions where session_id='$PROJECT_OWNER_SESSION' and action_name='cmd.rg.project_owner_approval' and status='pendingApproval'"
[[ "$(sql "select count(*) from paused_actions where session_id='$PROJECT_OWNER_SESSION' and action_name='cmd.rg.project_owner_approval' and status='pendingApproval'")" -gt 0 ]]
printf 'project_owner_command_runs='; sql "select count(*) from command_runs cr join command_versions cv on cv.id=cr.command_version_id where cv.action_id='cmd.rg.project_owner_approval'"
[[ "$(sql "select count(*) from command_runs cr join command_versions cv on cv.id=cr.command_version_id where cv.action_id='cmd.rg.project_owner_approval'")" -eq 0 ]]

write_request /tmp/scoped-conflict-request.json add /tmp/scoped-project-seed.json
CONFLICT_REQ=$(create_internal_request /tmp/scoped-conflict-request.json)
CONFLICT_DECIDE=$(cargo run --quiet --bin robdex-agent-runtime -- command-registry requests decide --session "$ADMIN" "$CONFLICT_REQ" --status approved --final-scope global --final-policy allow --final-command-file /tmp/scoped-project-seed.json 2>&1 || true)
printf 'scoped_conflict_decide=%s\n' "$CONFLICT_DECIDE" | rg 'scoped command action conflict'
printf 'scoped_conflict_status='; sql "select approval_status || '/' || application_status from command_registry_requests where id='$CONFLICT_REQ'"
[[ "$(sql "select approval_status || '/' || application_status from command_registry_requests where id='$CONFLICT_REQ'")" == "pending/pending" ]]

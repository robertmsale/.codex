#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/lib-validation-db.sh"
validation_setup_database
run() { printf '\n$ %s\n' "$*"; "$@"; }
sql() { psql "$ROBDEX_AGENT_RUNTIME_DATABASE_URL" -Atc "$1"; }
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
'{{"validationOnly":true,"source":"validate-action-resume.sh"}}'::jsonb,
{lit(data.get('rationale', 'validation internal resume request'))},
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

run cargo run --quiet -- init-db
run cargo run --quiet -- roles import-seeds

ADMIN_SESSION=$(cargo run --quiet -- sessions new --role runtime-allow)
cat >/tmp/agent-runtime-resume-rg.json <<'JSON'
{
  "operation": "add",
  "rationale": "validation owner approval registry command",
  "recommendedPolicy": "advisory only",
  "requester": "validation-script",
  "command": {
    "actionId": "cmd.rg.resume_approval",
    "binaryName": "rg",
    "candidatePaths": ["/opt/homebrew/bin/rg", "/usr/local/bin/rg", "/usr/bin/rg"],
    "starlarkObject": "rg_resume_approval",
    "starlarkMethod": "run",
    "argvPrefix": [],
    "allowArgsArg": true,
    "allowCwdArg": true,
    "defaultCwd": ".",
    "cwdPolicy": "underExecutionRoot",
    "envPolicy": "empty",
    "syncAllowed": true,
    "asyncAllowed": true,
    "maxRuntimeMs": 5000,
    "endOfTurnBehavior": "terminate",
    "stdinPolicy": "forbid",
    "minAwaitMs": 0,
    "maxAwaitMs": 60000,
    "outputBufferBytes": 64000,
    "terminateGraceMs": 1000,
    "outputLimitBytes": 12000,
    "mutationClass": "readOnly",
    "modelDescription": "validation command requiring owner approval"
  }
}
JSON
REGISTRY_REQUEST=$(create_internal_request /tmp/agent-runtime-resume-rg.json)
run cargo run --quiet -- command-registry requests decide --session "$ADMIN_SESSION" "$REGISTRY_REQUEST" --status approved --final-scope global --final-policy ownerApproval --final-command-file /tmp/agent-runtime-resume-rg.json
run cargo run --quiet -- command-registry requests apply --session "$ADMIN_SESSION" "$REGISTRY_REQUEST"

APPROVE_SESSION=$(cargo run --quiet -- sessions new --role runtime-allow)
PENDING_SESSION=$(cargo run --quiet -- sessions new --role runtime-allow)
DENIED_SESSION=$(cargo run --quiet -- sessions new --role runtime-allow)
printf '\n[sessions]\nAPPROVE_SESSION=%s\nPENDING_SESSION=%s\nDENIED_SESSION=%s\n' "$APPROVE_SESSION" "$PENDING_SESSION" "$DENIED_SESSION"

run cargo run --quiet -- send --session "$APPROVE_SESSION" --message 'Use execute_code with exactly this Starlark source: text = fs.read("Cargo.toml"); matches = cmd["rg_resume_approval"].run(args=["--files", "-g", "Cargo.toml"], cwd=".").sync(); output("approval should pause")'
APPROVAL_ID=$(sql "select id from approval_requests where session_id='$APPROVE_SESSION' and action_name='cmd.rg.resume_approval' order by created_at desc limit 1")
PAUSED_ID=$(sql "select id from paused_actions where approval_request_id='$APPROVAL_ID'")
printf '\n[created approval and paused action]\nAPPROVAL_ID=%s\nPAUSED_ID=%s\n' "$APPROVAL_ID" "$PAUSED_ID"
printf 'pre_resume_counts='; sql "select jsonb_build_object('approvals', (select count(*) from approval_requests where id='$APPROVAL_ID'), 'paused', (select count(*) from paused_actions where approval_request_id='$APPROVAL_ID'), 'commands', count(*) filter (where event_type='command.completed')) from event_stream where session_id='$APPROVE_SESSION'"
printf '\n[approvals show linked paused action]\n'; cargo run --quiet -- approvals show "$APPROVAL_ID" | python3 -c 'import json,sys; d=json.load(sys.stdin); p=d["pausedActions"][0]; print(d["status"], p["status"], p["actionName"], p["actionInput"]["argv"], p["actionInput"]["cwd"])'

run cargo run --quiet -- approvals decide "$APPROVAL_ID" --decision approved --reason 'validation approval'
printf 'after_decide_no_command='; sql "select jsonb_build_object('requestStatus', (select status from approval_requests where id='$APPROVAL_ID'), 'pausedStatus', (select status from paused_actions where id='$PAUSED_ID'), 'commands', count(*) filter (where event_type='command.completed')) from event_stream where session_id='$APPROVE_SESSION'"
run cargo run --quiet -- approvals resume "$APPROVAL_ID"
printf 'after_resume='; sql "select jsonb_build_object('pausedStatus', (select status from paused_actions where id='$PAUSED_ID'), 'commands', count(*) filter (where event_type='command.completed'), 'resumeStarted', count(*) filter (where event_type='approval.resume.started'), 'resumePolicy', count(*) filter (where event_type='policy.resumeDecision'), 'resumeCompleted', count(*) filter (where event_type='approval.resume.completed')) from event_stream where session_id='$APPROVE_SESSION'"
printf 'resume_event_order='; sql "select string_agg(event_type, ' > ' order by sequence) from event_stream where session_id='$APPROVE_SESSION' and event_type in ('approval.resume.started','policy.resumeDecision','command.completed','approval.resume.completed')"
printf 'original_turn_status='; sql "select status from turns where session_id='$APPROVE_SESSION' order by started_at desc limit 1"
set +e
SECOND=$(cargo run --quiet -- approvals resume "$APPROVAL_ID" 2>&1)
SECOND_STATUS=$?
set -e
printf 'second_resume_status=%s\n' "$SECOND_STATUS"
printf '%s\n' "$SECOND" | rg 'paused action is not resume-ready'

ASYNC_SESSION=$(cargo run --quiet -- sessions new --role runtime-allow)
run cargo run --quiet -- send --session "$ASYNC_SESSION" --message 'Use execute_code with exactly this Starlark source: h = cmd["rg_resume_approval"].run(args=["--files", "-g", "Cargo.toml"], cwd=".").start(); output("async approval should pause")'
ASYNC_APPROVAL=$(sql "select id from approval_requests where session_id='$ASYNC_SESSION' and action_name='cmd.rg.resume_approval' order by created_at desc limit 1")
ASYNC_PAUSED=$(sql "select id from paused_actions where approval_request_id='$ASYNC_APPROVAL'")
printf 'async_paused_mode='; sql "select action_input->>'executionMode' from paused_actions where id='$ASYNC_PAUSED'"
run cargo run --quiet -- approvals decide "$ASYNC_APPROVAL" --decision approved --reason 'validation async approval'
run cargo run --quiet -- approvals resume "$ASYNC_APPROVAL"
printf 'async_resume_processes='; sql "select count(*) from managed_processes where session_id='$ASYNC_SESSION' and metadata->>'resumed'='true'"
printf 'async_resume_no_command_run='; sql "select case when count(*) filter (where event_type='command.completed')=0 then 1 else 0 end from event_stream where session_id='$ASYNC_SESSION'"

run cargo run --quiet -- send --session "$PENDING_SESSION" --message 'Use execute_code with exactly this Starlark source: matches = cmd["rg_resume_approval"].run(args=["--files", "-g", "Cargo.toml"], cwd=".").sync(); output("pending should pause")'
PENDING_APPROVAL=$(sql "select id from approval_requests where session_id='$PENDING_SESSION' order by created_at desc limit 1")
set +e
PENDING_OUT=$(cargo run --quiet -- approvals resume "$PENDING_APPROVAL" 2>&1)
PENDING_STATUS=$?
set -e
printf 'pending_resume_status=%s\n' "$PENDING_STATUS"
printf '%s\n' "$PENDING_OUT" | rg 'approval request is not approved'

run cargo run --quiet -- send --session "$DENIED_SESSION" --message 'Use execute_code with exactly this Starlark source: matches = cmd["rg_resume_approval"].run(args=["--files", "-g", "Cargo.toml"], cwd=".").sync(); output("denied should pause")'
DENIED_APPROVAL=$(sql "select id from approval_requests where session_id='$DENIED_SESSION' order by created_at desc limit 1")
run cargo run --quiet -- approvals decide "$DENIED_APPROVAL" --decision denied --reason 'validation denial'
set +e
DENIED_OUT=$(cargo run --quiet -- approvals resume "$DENIED_APPROVAL" 2>&1)
DENIED_STATUS=$?
set -e
printf 'denied_resume_status=%s\n' "$DENIED_STATUS"
printf '%s\n' "$DENIED_OUT" | rg 'approval request is not approved'

printf '\n[validation complete]\n'

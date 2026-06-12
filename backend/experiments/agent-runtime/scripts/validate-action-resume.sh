#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/lib-validation-db.sh"
validation_setup_database
run() { printf '\n$ %s\n' "$*"; "$@"; }
sql() { psql "$ROBDEX_AGENT_RUNTIME_DATABASE_URL" -Atc "$1"; }

run cargo run --quiet -- init-db
run cargo run --quiet -- roles import-seeds

APPROVE_SESSION=$(cargo run --quiet -- new-session --role runtime-approval-rg)
PENDING_SESSION=$(cargo run --quiet -- new-session --role runtime-approval-rg)
DENIED_SESSION=$(cargo run --quiet -- new-session --role runtime-approval-rg)
printf '\n[sessions]\nAPPROVE_SESSION=%s\nPENDING_SESSION=%s\nDENIED_SESSION=%s\n' "$APPROVE_SESSION" "$PENDING_SESSION" "$DENIED_SESSION"

run cargo run --quiet -- send --session "$APPROVE_SESSION" --message 'Use execute_code with exactly this Starlark source: text = fs.read("Cargo.toml"); matches = cmd["rg"].run(args=["--files", "-g", "Cargo.toml"], cwd="."); output("approval should pause")'
APPROVAL_ID=$(sql "select id from approval_requests where session_id='$APPROVE_SESSION' and action_name='cmd.rg.run' order by created_at desc limit 1")
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

run cargo run --quiet -- send --session "$PENDING_SESSION" --message 'Use execute_code with exactly this Starlark source: matches = cmd["rg"].run(args=["--files", "-g", "Cargo.toml"], cwd="."); output("pending should pause")'
PENDING_APPROVAL=$(sql "select id from approval_requests where session_id='$PENDING_SESSION' order by created_at desc limit 1")
set +e
PENDING_OUT=$(cargo run --quiet -- approvals resume "$PENDING_APPROVAL" 2>&1)
PENDING_STATUS=$?
set -e
printf 'pending_resume_status=%s\n' "$PENDING_STATUS"
printf '%s\n' "$PENDING_OUT" | rg 'approval request is not approved'

run cargo run --quiet -- send --session "$DENIED_SESSION" --message 'Use execute_code with exactly this Starlark source: matches = cmd["rg"].run(args=["--files", "-g", "Cargo.toml"], cwd="."); output("denied should pause")'
DENIED_APPROVAL=$(sql "select id from approval_requests where session_id='$DENIED_SESSION' order by created_at desc limit 1")
run cargo run --quiet -- approvals decide "$DENIED_APPROVAL" --decision denied --reason 'validation denial'
set +e
DENIED_OUT=$(cargo run --quiet -- approvals resume "$DENIED_APPROVAL" 2>&1)
DENIED_STATUS=$?
set -e
printf 'denied_resume_status=%s\n' "$DENIED_STATUS"
printf '%s\n' "$DENIED_OUT" | rg 'approval request is not approved'

printf '\n[validation complete]\n'

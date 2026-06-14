#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/lib-validation-db.sh"
validation_setup_database

run() {
  printf '\n$ %s\n' "$*"
  "$@"
}

sql() {
  psql "$ROBDEX_AGENT_RUNTIME_DATABASE_URL" -Atc "$1"
}

assert_eq() {
  local label="$1"
  local expected="$2"
  local actual="$3"
  if [[ "$actual" != "$expected" ]]; then
    printf '%s expected %s got %s\n' "$label" "$expected" "$actual" >&2
    exit 1
  fi
}

ROOT="$(mktemp -d /tmp/robdex-agent-runtime-session.XXXXXX)"
cleanup_root() { rm -rf "$ROOT"; }
trap cleanup_root RETURN
printf 'session lifecycle root=%s\n' "$ROOT"

run cargo run --quiet --bin robdex-agent-runtime -- init-db
run cargo run --quiet --bin robdex-agent-runtime -- roles import-seeds

SESSION=$(cargo run --quiet --bin robdex-agent-runtime -- sessions new --role runtime-allow --project lifecycle --workdir "$ROOT" --worktree-root "$ROOT" --title 'Lifecycle Validation Session' --name lifecycle-validation)
printf 'session=%s\n' "$SESSION"
META=$(sql "select concat(worktree_root, '|', title, '|', name) from sessions where id='$SESSION'")
assert_eq session_metadata "$ROOT|Lifecycle Validation Session|lifecycle-validation" "$META"

run cargo run --quiet --bin robdex-agent-runtime -- sessions list
run cargo run --quiet --bin robdex-agent-runtime -- sessions show "$SESSION"

run cargo run --quiet --bin robdex-agent-runtime -- send --session "$SESSION" --message 'Use execute_code with exactly this Starlark source: fs.write("stored-workdir.txt", "stored workdir used"); output("created stored workdir file")'
test -f "$ROOT/stored-workdir.txt"
printf 'stored_workdir_file=%s\n' "$(cat "$ROOT/stored-workdir.txt")"
TURN=$(sql "select id from turns where session_id='$SESSION' and status='completed' order by started_at desc limit 1")
printf 'completed_turn=%s\n' "$TURN"
HISTORY_COUNT=$(sql "select count(*) from turns where session_id='$SESSION' and status='completed'")
assert_eq completed_history_count 1 "$HISTORY_COUNT"
run cargo run --quiet --bin robdex-agent-runtime -- sessions history "$SESSION"

BEFORE_INVALID_FORKS=$(sql "select count(*) from sessions")
set +e
MISSING_FORK=$(cargo run --quiet --bin robdex-agent-runtime -- sessions fork "$SESSION" --at-turn 00000000-0000-0000-0000-000000000001 2>&1)
MISSING_FORK_STATUS=$?
set -e
printf 'missing_fork_status=%s\n%s\n' "$MISSING_FORK_STATUS" "$MISSING_FORK"
AFTER_MISSING_FORK=$(sql "select count(*) from sessions")
assert_eq missing_fork_no_partial "$BEFORE_INVALID_FORKS" "$AFTER_MISSING_FORK"

RUNNING_TURN_ID="$(uuidgen | tr '[:upper:]' '[:lower:]')"
sql "insert into turns (id, session_id, role, input_text, status) values ('$RUNNING_TURN_ID', '$SESSION', 'user', 'running fork invalid', 'running')" >/dev/null
set +e
RUNNING_FORK=$(cargo run --quiet --bin robdex-agent-runtime -- sessions fork "$SESSION" --at-turn "$RUNNING_TURN_ID" 2>&1)
RUNNING_FORK_STATUS=$?
set -e
printf 'running_fork_status=%s\n%s\n' "$RUNNING_FORK_STATUS" "$RUNNING_FORK"
AFTER_RUNNING_FORK=$(sql "select count(*) from sessions")
assert_eq running_fork_no_partial "$BEFORE_INVALID_FORKS" "$AFTER_RUNNING_FORK"

FORK=$(cargo run --quiet --bin robdex-agent-runtime -- sessions fork "$SESSION" --at-turn "$TURN")
printf 'fork=%s\n' "$FORK"
FORK_PARENT=$(sql "select forked_from_session_id::text from sessions where id='$FORK'")
assert_eq fork_parent "$SESSION" "$FORK_PARENT"
FORK_HISTORY_BEFORE=$(cargo run --quiet --bin robdex-agent-runtime -- sessions history "$FORK")
printf '%s\n' "$FORK_HISTORY_BEFORE" | rg 'stored-workdir'

run cargo run --quiet --bin robdex-agent-runtime -- send --session "$SESSION" --message 'Use execute_code with exactly this Starlark source: fs.write("source-after-fork.txt", "source only"); output("source later")'
FORK_HISTORY_AFTER_SOURCE=$(cargo run --quiet --bin robdex-agent-runtime -- sessions history "$FORK")
if printf '%s\n' "$FORK_HISTORY_AFTER_SOURCE" | rg 'source-after-fork'; then
  printf 'fork history included source turn after fork boundary\n' >&2
  exit 1
fi

run cargo run --quiet --bin robdex-agent-runtime -- send --session "$FORK" --message 'Use execute_code with exactly this Starlark source: fs.write("fork-own.txt", "fork own"); output("fork own turn")'
FORK_HISTORY_AFTER=$(cargo run --quiet --bin robdex-agent-runtime -- sessions history "$FORK")
printf '%s\n' "$FORK_HISTORY_AFTER" | rg 'fork-own'

ARCHIVE=$(cargo run --quiet --bin robdex-agent-runtime -- sessions new --role runtime-allow --project lifecycle --workdir "$ROOT")
run cargo run --quiet --bin robdex-agent-runtime -- sessions archive "$ARCHIVE"
DEFAULT_LIST=$(cargo run --quiet --bin robdex-agent-runtime -- sessions list)
if printf '%s\n' "$DEFAULT_LIST" | rg "$ARCHIVE"; then
  printf 'archived session appeared in default list\n' >&2
  exit 1
fi
ALL_LIST=$(cargo run --quiet --bin robdex-agent-runtime -- sessions list --all)
printf '%s\n' "$ALL_LIST" | rg "$ARCHIVE"
run cargo run --quiet --bin robdex-agent-runtime -- sessions show "$ARCHIVE"

CLOSE=$(cargo run --quiet --bin robdex-agent-runtime -- sessions new --role runtime-allow --project lifecycle --workdir "$ROOT")
STALE_PROCESS_ID="$(uuidgen | tr '[:upper:]' '[:lower:]')"
sql "insert into managed_processes (id, handle, session_id, binary_name, argv, cwd, status, end_of_turn_behavior, end_of_session_behavior, metadata) values ('$STALE_PROCESS_ID', 'close-stale-handle', '$CLOSE', 'rg', '[]'::jsonb, '$ROOT', 'running', 'continue', 'block', '{}'::jsonb)" >/dev/null
set +e
BLOCKED_CLOSE=$(cargo run --quiet --bin robdex-agent-runtime -- sessions close "$CLOSE" --reason 'lifecycle validation blocked close' 2>&1)
BLOCKED_CLOSE_STATUS=$?
set -e
printf 'blocked_close_status=%s\n%s\n' "$BLOCKED_CLOSE_STATUS" "$BLOCKED_CLOSE"
CLOSE_SESSION_STATUS=$(sql "select status from sessions where id='$CLOSE'")
assert_eq blocked_close_session_open open "$CLOSE_SESSION_STATUS"
CLOSE_PROCESS_STATUS=$(sql "select status from managed_processes where session_id='$CLOSE' and handle='close-stale-handle'")
assert_eq blocked_close_process_running running "$CLOSE_PROCESS_STATUS"
CLOSE_BLOCKED_EVENTS=$(sql "select count(*) from event_stream where session_id='$CLOSE' and event_type='session.closeBlocked'")
assert_eq close_blocked_event 1 "$CLOSE_BLOCKED_EVENTS"
CLOSE_CLOSED_EVENTS=$(sql "select count(*) from event_stream where session_id='$CLOSE' and event_type='session.closed'")
assert_eq blocked_close_no_closed_event 0 "$CLOSE_CLOSED_EVENTS"
sql "update managed_processes set status='terminated', end_time=now(), termination_reason='validationCleanup' where session_id='$CLOSE' and handle='close-stale-handle'" >/dev/null
run cargo run --quiet --bin robdex-agent-runtime -- sessions close "$CLOSE" --reason 'lifecycle validation close'
CLOSE_CLOSED_EVENTS_AFTER=$(sql "select count(*) from event_stream where session_id='$CLOSE' and event_type='session.closed'")
assert_eq successful_close_event 1 "$CLOSE_CLOSED_EVENTS_AFTER"
set +e
CLOSED_SEND=$(cargo run --quiet --bin robdex-agent-runtime -- send --session "$CLOSE" --message 'This must be rejected before turn creation.' 2>&1)
CLOSED_STATUS=$?
set -e
printf 'closed_send_status=%s\n' "$CLOSED_STATUS"
printf '%s\n' "$CLOSED_SEND" | rg 'not open|session close'
CLOSED_TURNS=$(sql "select count(*) from turns where session_id='$CLOSE'")
assert_eq closed_turns 0 "$CLOSED_TURNS"

DETACHED_TERMINATE=$(cargo run --quiet --bin robdex-agent-runtime -- sessions new --role runtime-allow --project lifecycle --workdir "$ROOT")
DETACHED_PROCESS_ID="$(uuidgen | tr '[:upper:]' '[:lower:]')"
sql "insert into managed_processes (id, handle, session_id, binary_name, argv, cwd, status, end_of_turn_behavior, end_of_session_behavior, metadata) values ('$DETACHED_PROCESS_ID', 'detached-terminate-handle', '$DETACHED_TERMINATE', 'rg', '[]'::jsonb, '$ROOT', 'running', 'continue', 'terminate', '{}'::jsonb)" >/dev/null
set +e
DETACHED_CLOSE=$(cargo run --quiet --bin robdex-agent-runtime -- sessions close "$DETACHED_TERMINATE" --reason 'detached terminable should block' 2>&1)
DETACHED_CLOSE_STATUS=$?
set -e
printf 'detached_terminable_close_status=%s\n%s\n' "$DETACHED_CLOSE_STATUS" "$DETACHED_CLOSE"
DETACHED_SESSION_STATUS=$(sql "select status from sessions where id='$DETACHED_TERMINATE'")
assert_eq detached_terminable_remains_open open "$DETACHED_SESSION_STATUS"
DETACHED_CLOSED_EVENTS=$(sql "select count(*) from event_stream where session_id='$DETACHED_TERMINATE' and event_type='session.closed'")
assert_eq detached_terminable_no_closed_event 0 "$DETACHED_CLOSED_EVENTS"

APPROVAL_SESSION=$(cargo run --quiet --bin robdex-agent-runtime -- sessions new --role runtime-approval-rg --project lifecycle --workdir "$ROOT")
run cargo run --quiet --bin robdex-agent-runtime -- send --session "$APPROVAL_SESSION" --message 'Use execute_code with exactly this Starlark source: fs.write("approval-paused.txt", "paused"); output("paused")'
APPROVALS=$(sql "select count(*) from approval_requests where session_id='$APPROVAL_SESSION' and status='pending'")
PAUSED=$(sql "select count(*) from paused_actions where session_id='$APPROVAL_SESSION' and status='pendingApproval'")
printf 'pending_approvals=%s paused_actions=%s\n' "$APPROVALS" "$PAUSED"
assert_eq pending_approvals 1 "$APPROVALS"
assert_eq paused_actions 1 "$PAUSED"
SHOW_APPROVAL=$(cargo run --quiet --bin robdex-agent-runtime -- sessions show "$APPROVAL_SESSION")
printf '%s\n' "$SHOW_APPROVAL" | rg '"pendingApprovals": 1'
printf '%s\n' "$SHOW_APPROVAL" | rg '"pausedActions": 1'

RECOVERY_SESSION=$(cargo run --quiet --bin robdex-agent-runtime -- sessions new --role runtime-allow --project lifecycle --workdir "$ROOT")
RECOVERY_PROCESS_ID="$(uuidgen | tr '[:upper:]' '[:lower:]')"
sql "insert into managed_processes (id, handle, session_id, binary_name, argv, cwd, status, end_of_turn_behavior, metadata) values ('$RECOVERY_PROCESS_ID', 'startup-lost-handle', '$RECOVERY_SESSION', 'rg', '[]'::jsonb, '$ROOT', 'running', 'continue', '{}'::jsonb)" >/dev/null
run cargo run --quiet --bin robdex-agent-runtime -- init-db
RECOVERY_LOST=$(sql "select count(*) from managed_processes where session_id='$RECOVERY_SESSION' and handle='startup-lost-handle' and status='lost' and termination_reason='runtimeRestart'")
RECOVERY_DEGRADED=$(sql "select count(*) from event_stream where session_id='$RECOVERY_SESSION' and event_type='session.recoveryDegraded'")
printf 'startup_recovery_lost=%s degraded_events=%s\n' "$RECOVERY_LOST" "$RECOVERY_DEGRADED"
assert_eq startup_recovery_lost 1 "$RECOVERY_LOST"
assert_eq startup_recovery_degraded 1 "$RECOVERY_DEGRADED"

MODEL_HISTORY_EVENT_COUNT=$(sql "select count(*) from event_stream where session_id='$FORK' and event_type='model.tool_call' and payload::text like '%reconstructed_session_history%'")
printf 'model_history_event_count=%s\n' "$MODEL_HISTORY_EVENT_COUNT"
if [[ "$MODEL_HISTORY_EVENT_COUNT" -lt 1 ]]; then
  printf 'model request did not record reconstructed history evidence\n' >&2
  exit 1
fi
MODEL_HISTORY_ITEMS=$(sql "select coalesce((payload->'request'->'history'->>'items')::int, 0) from event_stream where session_id='$FORK' and event_type='model.tool_call' order by sequence desc limit 1")
MODEL_HISTORY_SOURCE=$(sql "select payload->'request'->'history'->>'source' from event_stream where session_id='$FORK' and event_type='model.tool_call' order by sequence desc limit 1")
MODEL_EVENT_FULL_INPUTS=$(sql "select count(*) from model_events where session_id='$FORK' and event_type='assistant_message' and payload->'request' ? 'input'")
MODEL_EVENT_SYNTHETIC_TEXT=$(sql "select count(*) from model_events where session_id='$FORK' and event_type='assistant_message' and payload::text like '%Runtime command context%'")
printf 'model_history_items=%s source=%s full_inputs=%s synthetic_text=%s\n' "$MODEL_HISTORY_ITEMS" "$MODEL_HISTORY_SOURCE" "$MODEL_EVENT_FULL_INPUTS" "$MODEL_EVENT_SYNTHETIC_TEXT"
if [[ "$MODEL_HISTORY_ITEMS" -lt 1 || "$MODEL_HISTORY_SOURCE" != "reconstructed_session_history" ]]; then
  printf 'model request did not record bounded reconstructed history evidence\n' >&2
  exit 1
fi
if [[ "$MODEL_EVENT_FULL_INPUTS" != "0" || "$MODEL_EVENT_SYNTHETIC_TEXT" != "0" ]]; then
  printf 'model_events persisted full request input or synthetic runtime command-context text\n' >&2
  exit 1
fi

printf '\nsession lifecycle validation passed\n'

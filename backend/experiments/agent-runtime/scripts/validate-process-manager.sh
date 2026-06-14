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

assert_positive() {
  local label="$1"
  local value="$2"
  if [[ "$value" -lt 1 ]]; then
    echo "$label expected a positive count, got $value" >&2
    exit 1
  fi
}

assert_one() {
  local label="$1"
  local value="$2"
  if [[ "$value" != "1" ]]; then
    echo "$label expected 1, got $value" >&2
    exit 1
  fi
}

run cargo run --quiet --bin robdex-agent-runtime -- init-db
run cargo run --quiet --bin robdex-agent-runtime -- roles import-seeds
SEEDED_MAX=$(sql "select count(*) from command_versions where action_id in ('cmd.rg.run','cmd.git.status','cmd.git.diff','cmd.cargo.check') and config->'maxRuntimeMs' <> 'null'::jsonb")
printf 'seeded_default_max_runtime_count=%s\n' "$SEEDED_MAX"
if [[ "$SEEDED_MAX" != "0" ]]; then
  echo "seeded default commands must not carry arbitrary maxRuntimeMs values" >&2
  exit 1
fi

SESSION=$(cargo run --quiet --bin robdex-agent-runtime -- sessions new --role runtime-allow)
printf 'process_session=%s\n' "$SESSION"

run cargo run --quiet --bin robdex-agent-runtime -- send --session "$SESSION" --message 'Use execute_code with exactly this Starlark source: h = cmd["rg"].run(args=["--files", "-g", "Cargo.toml"], cwd=".").start(); before = proc[h].is_running(); proc[h].await_for(mins=1); first = proc[h].flush_buffer(); second = proc[h].flush_buffer(); output("handle=" + h + " running=" + str(before) + " first=" + first + " second=" + second)'
MANAGED=$(sql "select count(*) from managed_processes where session_id='$SESSION'")
CHUNKS=$(sql "select count(*) from process_output_chunks")
EVENTS=$(sql "select count(*) from event_stream where session_id='$SESSION' and event_type in ('process.started','process.awaited','process.flushed','process.output')")
FLUSH_OUTPUT=$(sql "select count(*) from tool_calls where session_id='$SESSION' and result::text like '%Cargo.toml%' and result::text like '%second=%'")
FIRST_FLUSH_HAS_OUTPUT=$(sql "select count(*) from process_output_chunks where content like '%Cargo.toml%'")
SECOND_FLUSH_OMITS_PRIOR=$(sql "select count(*) from process_output_chunks where content = ''")
CURSOR_ADVANCES=$(sql "select count(*) from event_stream where session_id='$SESSION' and event_type='process.flushed' and (payload->'payload'->'details'->>'stdoutBytes')::int = 0")
printf 'managed_processes=%s\nprocess_chunks=%s\nprocess_events=%s\n' "$MANAGED" "$CHUNKS" "$EVENTS"
assert_positive managed_processes "$MANAGED"
assert_positive process_chunks "$CHUNKS"
assert_positive process_events "$EVENTS"
printf 'flush_cursor_output=%s\n' "$FLUSH_OUTPUT"
assert_positive flush_cursor_output "$FLUSH_OUTPUT"
printf 'first_flush_has_output=%s\nsecond_flush_omits_prior=%s\ncursor_advances=%s\n' "$FIRST_FLUSH_HAS_OUTPUT" "$SECOND_FLUSH_OMITS_PRIOR" "$CURSOR_ADVANCES"
assert_positive first_flush_has_output "$FIRST_FLUSH_HAS_OUTPUT"
assert_positive second_flush_omits_prior "$SECOND_FLUSH_OMITS_PRIOR"
assert_positive cursor_advances "$CURSOR_ADVANCES"

run cargo run --quiet --bin robdex-agent-runtime -- send --session "$SESSION" --message 'Use execute_code with exactly this Starlark source: h = cmd["cargo"].check(args=[]).start(); output("started " + h)'
EOT_CLEANUP=$(sql "select count(*) from event_stream where session_id='$SESSION' and event_type='process.endOfTurnCleanup'")
printf 'end_of_turn_cleanup=%s\n' "$EOT_CLEANUP"
assert_positive end_of_turn_cleanup "$EOT_CLEANUP"

run cargo run --quiet --bin robdex-agent-runtime -- send --session "$SESSION" --message 'Use execute_code with exactly this Starlark source: h = cmd["cargo"].check(args=[]).start(); proc[h].terminate(); output("terminated " + h)'
TERMINATED=$(sql "select count(*) from event_stream where session_id='$SESSION' and event_type='process.terminated' and status='terminated'")
printf 'proc_terminate_events=%s\n' "$TERMINATED"
assert_positive proc_terminate_events "$TERMINATED"

run cargo run --quiet --bin robdex-agent-runtime -- send --session "$SESSION" --message 'Use execute_code with exactly this Starlark source: h = cmd["rg"].run(args=["--files", "-g", "Cargo.toml"], cwd=".").start(); proc[h].input("forbidden"); output("bad")' || true
STDIN_REJECTIONS=$(sql "select count(*) from tool_calls where session_id='$SESSION' and status='failed' and result::text like '%stdinPolicy forbids input%'")
printf 'stdin_rejections=%s\n' "$STDIN_REJECTIONS"
assert_positive stdin_rejections "$STDIN_REJECTIONS"

sql "update command_versions set config=jsonb_set(config, '{maxRuntimeMs}', '1'::jsonb) where action_id='cmd.cargo.check'"
run cargo run --quiet --bin robdex-agent-runtime -- send --session "$SESSION" --message 'Use execute_code with exactly this Starlark source: cmd["cargo"].check(args=[]).sync(); output("bad")' || true
MAX_RUNTIME=$(sql "select count(*) from command_runs where status='maxRuntimeExceeded'")
MAX_RUNTIME_PG=$(sql "select count(*) from event_stream where event_type='command.completed' and payload->'processGroupTermination'->>'attempted'='true' and payload->'processGroupTermination'->>'reason'='maxRuntimeExceeded'")
printf 'max_runtime_sync=%s\n' "$MAX_RUNTIME"
assert_positive max_runtime_sync "$MAX_RUNTIME"
printf 'max_runtime_process_group_evidence=%s\n' "$MAX_RUNTIME_PG"
assert_positive max_runtime_process_group_evidence "$MAX_RUNTIME_PG"

sql "update command_versions set config=jsonb_set(config, '{maxRuntimeMs}', 'null'::jsonb) where action_id='cmd.rg.run'"
run cargo run --quiet --bin robdex-agent-runtime -- send --session "$SESSION" --message 'Use execute_code with exactly this Starlark source: files = cmd["rg"].run(args=["--files", "-g", "Cargo.toml"], cwd=".").sync(); output(files)'
NULL_COMPLETIONS=$(sql "select count(*) from command_runs where max_runtime_ms is null and status='completed'")
printf 'null_max_runtime_completions=%s\n' "$NULL_COMPLETIONS"
assert_positive null_max_runtime_completions "$NULL_COMPLETIONS"

sql "update command_versions set config=jsonb_set(jsonb_set(config, '{maxRuntimeMs}', '1000'::jsonb), '{endOfTurnBehavior}', '\"continue\"'::jsonb) where action_id='cmd.rg.run'"
run cargo run --quiet --bin robdex-agent-runtime -- send --session "$SESSION" --message 'Use execute_code with exactly this Starlark source: h = cmd["rg"].run(args=["--files", "-g", "Cargo.toml"], cwd=".").start(); output("quick finite " + h)'
sleep 2
QUICK_NATURAL=$(sql "select count(*) from managed_processes where session_id='$SESSION' and binary_name='rg' and status='completed' and termination_reason='naturalExit'")
QUICK_NATURAL_EVENT=$(sql "select count(*) from event_stream where session_id='$SESSION' and event_type='process.naturalExit' and status='completed'")
QUICK_FALSE_MAX=$(sql "select count(*) from managed_processes where session_id='$SESSION' and binary_name='rg' and status='maxRuntimeExceeded' and metadata->>'maxRuntimeSupervisor'='true'")
printf 'quick_finite_natural_status=%s\nquick_finite_natural_events=%s\nquick_finite_false_max=%s\n' "$QUICK_NATURAL" "$QUICK_NATURAL_EVENT" "$QUICK_FALSE_MAX"
assert_positive quick_finite_natural_status "$QUICK_NATURAL"
assert_positive quick_finite_natural_events "$QUICK_NATURAL_EVENT"
if [[ "$QUICK_FALSE_MAX" != "0" ]]; then
  echo "quick finite async command was falsely marked maxRuntimeExceeded" >&2
  exit 1
fi

sql "update command_versions set config=jsonb_set(jsonb_set(config, '{maxRuntimeMs}', '100'::jsonb), '{endOfTurnBehavior}', '\"continue\"'::jsonb) where action_id='cmd.cargo.check'"
run cargo run --quiet --bin robdex-agent-runtime -- send --session "$SESSION" --message 'Use execute_code with exactly this Starlark source: h = cmd["cargo"].check(args=[]).start(); output("durable max runtime " + h)'
sleep 1
DURABLE_MAX=$(sql "select count(*) from managed_processes where session_id='$SESSION' and status='maxRuntimeExceeded' and termination_reason='maxRuntimeExceeded'")
DURABLE_EVENT=$(sql "select count(*) from event_stream where session_id='$SESSION' and event_type='process.maxRuntimeExceeded' and status='maxRuntimeExceeded'")
printf 'async_max_runtime_durable_status=%s\nasync_max_runtime_durable_events=%s\n' "$DURABLE_MAX" "$DURABLE_EVENT"
assert_positive async_max_runtime_durable_status "$DURABLE_MAX"
assert_positive async_max_runtime_durable_events "$DURABLE_EVENT"

sql "insert into managed_processes (id, handle, session_id, starting_turn_id, binary_name, argv, cwd, status, end_of_turn_behavior) values (gen_random_uuid(), 'proc_reconcile_validation', '$SESSION', null, 'rg', '[]'::jsonb, '.', 'running', 'continue')"
run cargo run --quiet --bin robdex-agent-runtime -- init-db
STARTUP_LOST=$(sql "select count(*) from managed_processes where handle='proc_reconcile_validation' and status='lost' and termination_reason='runtimeRestart'")
printf 'startup_lost=%s\n' "$STARTUP_LOST"
assert_one startup_lost "$STARTUP_LOST"

sql "update command_versions set config=jsonb_set(config, '{executionPolicy}', '\"ownerApproval\"'::jsonb) where action_id='cmd.rg.run'"
BEFORE_APPROVAL_RUNS=$(sql "select count(*) from command_runs")
run cargo run --quiet --bin robdex-agent-runtime -- send --session "$SESSION" --message 'Use execute_code with exactly this Starlark source: h = cmd["rg"].run(args=["--files", "-g", "Cargo.toml"], cwd=".").start(); output("bad")' || true
APPROVALS=$(sql "select count(*) from approval_requests where session_id='$SESSION' and action_name='cmd.rg.run' and status='pending'")
PAUSED=$(sql "select count(*) from paused_actions where session_id='$SESSION' and action_name='cmd.rg.run' and status='pendingApproval'")
NO_NEW_RUNS=$(sql "select case when count(*)=$BEFORE_APPROVAL_RUNS then 1 else 0 end from command_runs")
printf 'approval_requests=%s\npaused_actions=%s\napproval_no_new_command_runs=%s\n' "$APPROVALS" "$PAUSED" "$NO_NEW_RUNS"
assert_positive approval_requests "$APPROVALS"
assert_positive paused_actions "$PAUSED"
assert_one approval_no_new_command_runs "$NO_NEW_RUNS"

printf '\n[process validation complete]\n'

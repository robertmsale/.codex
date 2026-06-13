#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
source scripts/lib-validation-db.sh
validation_setup_database

export ROBDEX_AGENT_RUNTIME_EMBEDDING_PROVIDER=deterministic
export ROBDEX_AGENT_RUNTIME_EMBEDDING_DIMENSIONS=2560

run() { printf '\n$ %s\n' "$*"; "$@"; }
sql() { psql "$ROBDEX_AGENT_RUNTIME_DATABASE_URL" -At -c "$1"; }
send_source() {
  local session="$1"
  local source="$2"
  run cargo run --quiet -- send --session "$session" --message "Use execute_code with exactly this Starlark source: $source"
}

if ! psql "$ROBDEX_AGENT_RUNTIME_DATABASE_URL" -At -c "select 1 from pg_available_extensions where name='vector'" | rg -q '^1$'; then
  printf 'pgvector extension package is not available in this Postgres installation; install pgvector before running workflow-memory validation.\n' >&2
  exit 2
fi

run cargo run --quiet -- init-db
run cargo run --quiet -- roles import-seeds

ROOT="$(mktemp -d /tmp/agent-runtime-workflow-memory.XXXXXX)"
printf 'workflow-memory-root=%s\n' "$ROOT"
printf 'seed\n' >"$ROOT/Cargo.toml"

SEED_SESSION=$(cargo run --quiet -- sessions new --role runtime-allow --project alpha --workdir "$ROOT")
FAIL_SESSION=$(cargo run --quiet -- sessions new --role runtime-allow --project alpha --workdir "$ROOT")
DENY_SESSION=$(cargo run --quiet -- sessions new --role runtime-no-rg --project alpha --workdir "$ROOT")
BETA_SESSION=$(cargo run --quiet -- sessions new --role runtime-allow --project beta --workdir "$ROOT")
printf 'sessions=%s,%s,%s,%s\n' "$SEED_SESSION" "$FAIL_SESSION" "$DENY_SESSION" "$BETA_SESSION"

PROMOTE_SOURCE='fs.write("memory-target.txt", "needle workflow memory success"); text = fs.read("memory-target.txt"); workflow_memory.remember_when(text == "needle workflow memory success", "Write memory target", "Use fs.write then fs.read to verify exact content after missing-file failures"); output("promoted")'
send_source "$SEED_SESSION" "$PROMOTE_SOURCE"
MEMORY_ID=$(sql "select id from workflow_memories where session_id='$SEED_SESSION' order by promoted_at desc limit 1")
printf 'promoted_memory=%s\n' "$MEMORY_ID"
[[ -n "$MEMORY_ID" ]]

send_source "$SEED_SESSION" "$PROMOTE_SOURCE"
printf 'exact_duplicate_memory_count='; sql "select count(*) from workflow_memories where session_id='$SEED_SESSION'"
[[ "$(sql "select count(*) from workflow_memories where session_id='$SEED_SESSION'")" -eq 1 ]]
printf 'duplicate_events='; sql "select count(*) from workflow_memory_events where session_id='$SEED_SESSION' and event_type='workflow_memory.duplicate_collapsed'"
[[ "$(sql "select count(*) from workflow_memory_events where session_id='$SEED_SESSION' and event_type='workflow_memory.duplicate_collapsed'")" -gt 0 ]]

send_source "$FAIL_SESSION" 'fs.read("missing-workflow-memory-file.txt"); output("unreachable")'
printf 'failed_script_embeddings='; sql "select count(*) from workflow_memory_script_embeddings where session_id='$FAIL_SESSION'"
[[ "$(sql "select count(*) from workflow_memory_script_embeddings where session_id='$FAIL_SESSION'")" -gt 0 ]]
printf 'failed_script_promotions='; sql "select count(*) from workflow_memories where session_id='$FAIL_SESSION'"
[[ "$(sql "select count(*) from workflow_memories where session_id='$FAIL_SESSION'")" -eq 0 ]]

send_source "$FAIL_SESSION" 'tips = workflow_memory.help(); output(tips)'
printf 'alpha_help_result_count='; sql "select payload->>'resultCount' from workflow_memory_events where session_id='$FAIL_SESSION' and event_type='workflow_memory.help' order by created_at desc limit 1"
[[ "$(sql "select payload->>'resultCount' from workflow_memory_events where session_id='$FAIL_SESSION' and event_type='workflow_memory.help' order by created_at desc limit 1")" -gt 0 ]]
printf 'alpha_help_mentions_memory='; sql "select count(*) from workflow_memory_events where session_id='$FAIL_SESSION' and event_type='workflow_memory.help' and payload::text like '%$MEMORY_ID%'"
[[ "$(sql "select count(*) from workflow_memory_events where session_id='$FAIL_SESSION' and event_type='workflow_memory.help' and payload::text like '%$MEMORY_ID%'")" -gt 0 ]]

send_source "$FAIL_SESSION" "workflow_memory.mark_attempted(\"$MEMORY_ID\", variant=True); workflow_memory.mark_not_helpful(\"$MEMORY_ID\", \"not enough context\"); output(\"feedback\")"
printf 'feedback_events='; sql "select count(*) from workflow_memory_events where memory_id='$MEMORY_ID' and event_type in ('workflow_memory.mark_attempted','workflow_memory.mark_not_helpful')"
[[ "$(sql "select count(*) from workflow_memory_events where memory_id='$MEMORY_ID' and event_type in ('workflow_memory.mark_attempted','workflow_memory.mark_not_helpful')")" -eq 2 ]]

DENY_OUT=$(cargo run --quiet -- send --session "$DENY_SESSION" --message 'Use execute_code with exactly this Starlark source: workflow_memory.remember_when(True, "Denied memory", "restrictive role should block"); output("denied")' 2>&1 || true)
printf 'deny_output=%s\n' "$DENY_OUT"
printf 'deny_policy_events='; sql "select count(*) from event_stream where session_id='$DENY_SESSION' and event_type='policy.decision' and status='deny' and payload->>'action'='workflow_memory.remember.project'"
[[ "$(sql "select count(*) from event_stream where session_id='$DENY_SESSION' and event_type='policy.decision' and status='deny' and payload->>'action'='workflow_memory.remember.project'")" -gt 0 ]]
printf 'deny_memories='; sql "select count(*) from workflow_memories where session_id='$DENY_SESSION'"
[[ "$(sql "select count(*) from workflow_memories where session_id='$DENY_SESSION'")" -eq 0 ]]

send_source "$BETA_SESSION" 'fs.read("missing-workflow-memory-file.txt"); output("unreachable")'
send_source "$BETA_SESSION" 'tips = workflow_memory.help(); output(tips)'
printf 'beta_help_result_count='; sql "select payload->>'resultCount' from workflow_memory_events where session_id='$BETA_SESSION' and event_type='workflow_memory.help' order by created_at desc limit 1"
[[ "$(sql "select payload->>'resultCount' from workflow_memory_events where session_id='$BETA_SESSION' and event_type='workflow_memory.help' order by created_at desc limit 1")" -eq 0 ]]

printf '\n[workflow memory validation complete]\n'

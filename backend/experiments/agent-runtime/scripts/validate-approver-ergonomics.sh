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
  psql "$ROBDEX_AGENT_RUNTIME_DATABASE_URL" -Atq -c "$1"
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

run cargo run --quiet --bin robdex-agent-runtime -- init-db
run cargo run --quiet --bin robdex-agent-runtime -- roles import-seeds
ADMIN=$(cargo run --quiet --bin robdex-agent-runtime -- sessions new --role runtime-allow --project ergonomics --workdir .)
printf 'admin_session=%s\n' "$ADMIN"

run cargo run --quiet --bin robdex-agent-runtime -- command-registry seed-requests --session "$ADMIN" --mode refresh
REQUEST_ID=$(sql "select id from command_registry_requests where operation='update' and approval_status='pending' order by created_at asc limit 1")
printf 'request_id=%s\n' "$REQUEST_ID"

REVIEW_BEFORE=$(cargo run --quiet --bin robdex-agent-runtime -- command-registry requests review "$REQUEST_ID")
printf '%s\n' "$REVIEW_BEFORE" | rg '"proposedCommand"'
printf '%s\n' "$REVIEW_BEFORE" | rg '"currentRegistryState"'
printf '%s\n' "$REVIEW_BEFORE" | rg '"risk"'
printf '%s\n' "$REVIEW_BEFORE" | rg '"semanticDiff"'
printf '%s\n' "$REVIEW_BEFORE" | rg '"readiness"'

TEMPLATE=/tmp/agent-runtime-approver-template.json
run cargo run --quiet --bin robdex-agent-runtime -- command-registry requests final-template "$REQUEST_ID" --out "$TEMPLATE"
test -s "$TEMPLATE"
python3 - "$TEMPLATE" <<'PY'
import json, sys
path = sys.argv[1]
data = json.load(open(path))
cmd = data["command"]
cmd["modelDescription"] = cmd["modelDescription"] + " (approver ergonomics validation)"
json.dump(data, open(path, "w"), indent=2)
PY

set +e
MISSING_SCOPE=$(cargo run --quiet --bin robdex-agent-runtime -- command-registry requests preview-decision "$REQUEST_ID" --status approved --final-policy allow --final-command-file "$TEMPLATE" 2>&1)
MISSING_SCOPE_STATUS=$?
set -e
printf 'missing_scope_status=%s\n%s\n' "$MISSING_SCOPE_STATUS" "$MISSING_SCOPE"
printf '%s\n' "$MISSING_SCOPE" | rg 'final scope'

set +e
MISSING_POLICY=$(cargo run --quiet --bin robdex-agent-runtime -- command-registry requests preview-decision "$REQUEST_ID" --status approved --final-scope global --final-command-file "$TEMPLATE" 2>&1)
MISSING_POLICY_STATUS=$?
set -e
printf 'missing_policy_status=%s\n%s\n' "$MISSING_POLICY_STATUS" "$MISSING_POLICY"
printf '%s\n' "$MISSING_POLICY" | rg 'final execution policy'

INVALID=/tmp/agent-runtime-approver-invalid.json
python3 - "$TEMPLATE" "$INVALID" <<'PY'
import json, sys
data = json.load(open(sys.argv[1]))
data["command"]["actionId"] = "invalid-action"
json.dump(data, open(sys.argv[2], "w"), indent=2)
PY
set +e
INVALID_OUT=$(cargo run --quiet --bin robdex-agent-runtime -- command-registry requests preview-decision "$REQUEST_ID" --status approved --final-scope global --final-policy allow --final-command-file "$INVALID" 2>&1)
INVALID_STATUS=$?
set -e
printf 'invalid_status=%s\n%s\n' "$INVALID_STATUS" "$INVALID_OUT"
printf '%s\n' "$INVALID_OUT" | rg 'registry command action'

PREVIEW=$(cargo run --quiet --bin robdex-agent-runtime -- command-registry requests preview-decision "$REQUEST_ID" --status approved --final-scope global --final-policy allow --final-command-file "$TEMPLATE")
printf '%s\n' "$PREVIEW" | rg '"mutation": false'
STATUS_AFTER_PREVIEW=$(sql "select approval_status || '/' || application_status from command_registry_requests where id='$REQUEST_ID'")
assert_eq preview_no_mutation 'pending/pending' "$STATUS_AFTER_PREVIEW"

run cargo run --quiet --bin robdex-agent-runtime -- command-registry requests decide --session "$ADMIN" "$REQUEST_ID" --status approved --final-scope global --final-policy allow --final-command-file "$TEMPLATE"
REVIEW_AFTER=$(cargo run --quiet --bin robdex-agent-runtime -- command-registry requests review "$REQUEST_ID")
printf '%s\n' "$REVIEW_AFTER" | rg '"finalCommand"'
printf '%s\n' "$REVIEW_AFTER" | rg '"proposedVsFinal"'
DECIDED_EVENTS=$(sql "select count(*) from event_stream where session_id='$ADMIN' and event_type='command_registry.decided'")
assert_eq decided_event 1 "$DECIDED_EVENTS"

run cargo run --quiet --bin robdex-agent-runtime -- command-registry requests apply --session "$ADMIN" "$REQUEST_ID"
APPLIED_EVENTS=$(sql "select count(*) from event_stream where session_id='$ADMIN' and event_type='command_registry.applied' and payload ? 'affectedCommandVersionId'")
assert_eq applied_event 1 "$APPLIED_EVENTS"
APPLIED_DIFF_EVENTS=$(sql "select count(*) from event_stream where session_id='$ADMIN' and event_type='command_registry.applied' and payload ? 'semanticDiff' and payload->'semanticDiff' ? 'proposedVsFinal' and payload->'semanticDiff' ? 'currentVsFinal'")
assert_eq applied_semantic_diff_event 1 "$APPLIED_DIFF_EVENTS"

DENIED_ID=$(sql "insert into command_registry_requests (id, session_id, operation, proposed_command, requester_context, rationale, recommended_policy, requester, requested_by_role, approval_status, application_status) select gen_random_uuid(), '$ADMIN', 'add', proposed_command, '{}'::jsonb, 'deny validation', 'deny validation', 'validation', '{}'::jsonb, 'pending', 'pending' from command_registry_requests where id='$REQUEST_ID' returning id")
run cargo run --quiet --bin robdex-agent-runtime -- command-registry requests decide --session "$ADMIN" "$DENIED_ID" --status denied
set +e
DENIED_APPLY=$(cargo run --quiet --bin robdex-agent-runtime -- command-registry requests apply --session "$ADMIN" "$DENIED_ID" 2>&1)
DENIED_APPLY_STATUS=$?
set -e
printf 'denied_apply_status=%s\n%s\n' "$DENIED_APPLY_STATUS" "$DENIED_APPLY"
printf '%s\n' "$DENIED_APPLY" | rg 'must be approved'

PROJECT_COMMAND=/tmp/agent-runtime-approver-project.json
python3 - "$TEMPLATE" "$PROJECT_COMMAND" <<'PY'
import json, sys
data = json.load(open(sys.argv[1]))
cmd = data["command"]
cmd["actionId"] = "cmd.rg.approver_conflict"
cmd["starlarkObject"] = "rg_approver_conflict"
cmd["modelDescription"] = "approver ergonomics project conflict helper"
json.dump(data, open(sys.argv[2], "w"), indent=2)
PY
PROJECT_REQ=$(sql "insert into command_registry_requests (id, session_id, operation, proposed_command, requester_context, rationale, recommended_policy, requester, requested_by_role, approval_status, application_status) values (gen_random_uuid(), '$ADMIN', 'add', (select (pg_read_file('$PROJECT_COMMAND'))::jsonb -> 'command'), '{}'::jsonb, 'project conflict setup', 'validation', 'validation', '{}'::jsonb, 'pending', 'pending') returning id")
run cargo run --quiet --bin robdex-agent-runtime -- command-registry requests decide --session "$ADMIN" "$PROJECT_REQ" --status approved --final-scope project --final-project ergonomics --final-policy allow --final-command-file "$PROJECT_COMMAND"
run cargo run --quiet --bin robdex-agent-runtime -- command-registry requests apply --session "$ADMIN" "$PROJECT_REQ"
GLOBAL_CONFLICT_REQ=$(sql "insert into command_registry_requests (id, session_id, operation, proposed_command, requester_context, rationale, recommended_policy, requester, requested_by_role, approval_status, application_status) values (gen_random_uuid(), '$ADMIN', 'add', (select (pg_read_file('$PROJECT_COMMAND'))::jsonb -> 'command'), '{}'::jsonb, 'global conflict validation', 'validation', 'validation', '{}'::jsonb, 'pending', 'pending') returning id")
set +e
CONFLICT_OUT=$(cargo run --quiet --bin robdex-agent-runtime -- command-registry requests preview-decision "$GLOBAL_CONFLICT_REQ" --status approved --final-scope global --final-policy allow --final-command-file "$PROJECT_COMMAND" 2>&1)
CONFLICT_STATUS=$?
set -e
printf 'conflict_status=%s\n%s\n' "$CONFLICT_STATUS" "$CONFLICT_OUT"
printf '%s\n' "$CONFLICT_OUT" | rg 'scoped command action conflict'
CONFLICT_STATE=$(sql "select approval_status || '/' || application_status from command_registry_requests where id='$GLOBAL_CONFLICT_REQ'")
assert_eq conflict_no_mutation 'pending/pending' "$CONFLICT_STATE"

APPROVAL_ID=$(sql "insert into approval_requests (id, session_id, action_name, requested_by_role, input_context, required_approver_kind, status) values (gen_random_uuid(), '$ADMIN', 'command_registry.apply', '{}'::jsonb, '{}'::jsonb, 'owner', 'pending') returning id")
LINKED_REQ=$(sql "insert into command_registry_requests (id, session_id, operation, proposed_command, requester_context, rationale, recommended_policy, requester, requested_by_role, approval_request_id, approval_status, application_status) select gen_random_uuid(), '$ADMIN', 'add', proposed_command, '{}'::jsonb, 'approval show validation', 'validation', 'validation', '{}'::jsonb, '$APPROVAL_ID', 'pending', 'pending' from command_registry_requests where id='$REQUEST_ID' returning id")
APPROVAL_SHOW=$(cargo run --quiet --bin robdex-agent-runtime -- approvals show "$APPROVAL_ID")
printf '%s\n' "$APPROVAL_SHOW" | rg '"commandRegistryRequests"'
printf '%s\n' "$APPROVAL_SHOW" | rg "$LINKED_REQ"

printf '\napprover ergonomics validation passed\n'

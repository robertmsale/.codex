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

run cargo run --quiet --bin robdex-agent-runtime -- init-db
run cargo run --quiet --bin robdex-agent-runtime -- roles import-seeds

printf '\n[roles list from DB]\n'
run cargo run --quiet --bin robdex-agent-runtime -- roles list

printf '\n[roles show from DB]\n'
cargo run --quiet --bin robdex-agent-runtime -- roles show runtime-allow | python3 -c 'import json,sys; d=json.load(sys.stdin); print(d["id"], d["version"], d["displayName"], "instr="+str(len(d["instructionText"])), "model="+d["modelDefaults"]["model"])'

ALLOW_SESSION=$(cargo run --quiet --bin robdex-agent-runtime -- sessions new --role runtime-allow)
DENY_SESSION=$(cargo run --quiet --bin robdex-agent-runtime -- sessions new --role runtime-no-rg)
APPROVAL_SESSION=$(cargo run --quiet --bin robdex-agent-runtime -- sessions new --role runtime-approval-rg)
printf '\n[sessions]\nALLOW_SESSION=%s\nDENY_SESSION=%s\nAPPROVAL_SESSION=%s\n' "$ALLOW_SESSION" "$DENY_SESSION" "$APPROVAL_SESSION"

printf '\n[session snapshots include instruction text]\n'
sql "select id || ' ' || role_id || ' ' || role_version || ' instr=' || length(role_snapshot->>'instructionText') from sessions where id in ('$ALLOW_SESSION','$DENY_SESSION','$APPROVAL_SESSION') order by created_at"

run cargo run --quiet --bin robdex-agent-runtime -- send --session "$ALLOW_SESSION" --message 'Use execute_code with exactly this Starlark source: text = fs.read("Cargo.toml"); print("db canonical instruction proof")'
ALLOW_TURN=$(sql "select id from turns where session_id='$ALLOW_SESSION' order by started_at desc limit 1")
printf '\n[model request uses session snapshot instructions]\n'
sql "select event_type || ':' || ((payload->'request'->'roleInstructions'->>'source')) || ':' || left((payload->'request'->'roleInstructions'->>'prefix'), 80) from event_stream where turn_id='$ALLOW_TURN' and event_type in ('model.tool_call','model.final_response') order by sequence"
printf '\n[sanitized model event request evidence]\n'
sql "select event_type || ':model=' || (payload->'request'->>'model') || ':tools=' || (payload->'request'->>'toolCount') || ':ctx=' || (payload->'commandContext'->>'id') from model_events where turn_id='$ALLOW_TURN' and event_type='assistant_message'"
FULL_INPUT_COUNT=$(sql "select count(*) from model_events where turn_id='$ALLOW_TURN' and event_type='assistant_message' and payload->'request' ? 'input'")
SYNTHETIC_TEXT_COUNT=$(sql "select count(*) from model_events where turn_id='$ALLOW_TURN' and event_type='assistant_message' and payload::text like '%Runtime command context%'")
printf 'full_input_count=%s synthetic_text_count=%s\n' "$FULL_INPUT_COUNT" "$SYNTHETIC_TEXT_COUNT"
if [[ "$FULL_INPUT_COUNT" != "0" || "$SYNTHETIC_TEXT_COUNT" != "0" ]]; then
  printf 'model_events persisted full request input or synthetic runtime command-context text\n' >&2
  exit 1
fi

TMPDIR=$(mktemp -d /tmp/agent-runtime-role-import.XXXXXX)
mkdir -p "$TMPDIR/prompts"
cp roles/prompts/runtime-allow.md "$TMPDIR/prompts/runtime-allow.md"
python3 - "$TMPDIR/runtime-allow-mutated.json" <<'PY'
import json, pathlib, sys
source = pathlib.Path('roles/runtime-allow.json')
out = pathlib.Path(sys.argv[1])
data = json.loads(source.read_text())
data['version'] = 'proof-mutated'
data['displayName'] = 'Runtime Allow Proof Mutated'
out.write_text(json.dumps(data, indent=2) + '\n')
PY
run cargo run --quiet --bin robdex-agent-runtime -- roles import "$TMPDIR/runtime-allow-mutated.json"
printf '\n[current role changed; old session snapshot unchanged]\n'
printf 'current='; cargo run --quiet --bin robdex-agent-runtime -- roles show runtime-allow | python3 -c 'import json,sys; d=json.load(sys.stdin); print(d["version"]+" "+d["displayName"])'
printf 'old_session='; sql "select (role_snapshot->>'version') || ' ' || (role_snapshot->>'displayName') from sessions where id='$ALLOW_SESSION'"
run cargo run --quiet --bin robdex-agent-runtime -- roles import roles/runtime-allow.json >/tmp/agent-runtime-role-restore.log
rm -rf "$TMPDIR"

run cargo run --quiet --bin robdex-agent-runtime -- send --session "$DENY_SESSION" --message 'Use execute_code with exactly this Starlark source: fs.write("tmp/db-role-deny.txt", "deny should not execute"); print("deny should not execute")'
run cargo run --quiet --bin robdex-agent-runtime -- send --session "$APPROVAL_SESSION" --message 'Use execute_code with exactly this Starlark source: fs.write("tmp/db-role-approval.txt", "approval should pause"); print("approval should not execute")'

printf '\n[denied action evidence]\n'
sql "select sequence || ':' || event_type || ':' || status || ':' || (payload->>'action') || ':' || (payload->>'decision') from event_stream where session_id='$DENY_SESSION' and event_type in ('policy.decision','command.completed') order by sequence"
printf 'deny_counts='; sql "select jsonb_build_object('commands', count(*) filter (where event_type='command.completed'), 'denies', count(*) filter (where event_type='policy.decision' and status='deny')) from event_stream where session_id='$DENY_SESSION'"

printf '\n[approvalRequired action evidence]\n'
sql "select sequence || ':' || event_type || ':' || status || ':' || (payload->>'action') || ':' || (payload->>'decision') from event_stream where session_id='$APPROVAL_SESSION' and event_type in ('policy.decision','command.completed') order by sequence"
printf 'approval_counts='; sql "select jsonb_build_object('commands', count(*) filter (where event_type='command.completed'), 'approvalRequired', count(*) filter (where event_type='policy.decision' and status='approvalRequired')) from event_stream where session_id='$APPROVAL_SESSION'"

printf '\n[validation complete]\n'

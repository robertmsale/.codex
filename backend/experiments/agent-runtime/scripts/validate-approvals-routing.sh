#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/lib-validation-db.sh"
validation_setup_database
run() { printf '\n$ %s\n' "$*"; "$@"; }
sql() { psql "$ROBDEX_AGENT_RUNTIME_DATABASE_URL" -Atc "$1"; }

run cargo run --quiet -- init-db
run cargo run --quiet -- roles import-seeds
run cargo run --quiet -- roles validate

ALLOW_SESSION=$(cargo run --quiet -- new-session --role runtime-allow)
DENY_SESSION=$(cargo run --quiet -- new-session --role runtime-no-rg)
APPROVAL_SESSION=$(cargo run --quiet -- new-session --role runtime-approval-rg)
printf '\n[sessions]\nALLOW_SESSION=%s\nDENY_SESSION=%s\nAPPROVAL_SESSION=%s\n' "$ALLOW_SESSION" "$DENY_SESSION" "$APPROVAL_SESSION"

run cargo run --quiet -- send --session "$APPROVAL_SESSION" --message 'Use execute_code with exactly this Starlark source: fs.write("tmp/approval-routing.txt", "approval should pause"); output("approval should not execute")'
APPROVAL_ID=$(sql "select id from approval_requests where session_id='$APPROVAL_SESSION' and action_name='fs.write' order by created_at desc limit 1")
printf '\n[approval request]\nAPPROVAL_ID=%s\n' "$APPROVAL_ID"
printf 'pre_decision_counts='; sql "select jsonb_build_object('approvalRequests', count(*) filter (where event_type='approval.requested'), 'commands', count(*) filter (where event_type='command.completed')) from event_stream where session_id='$APPROVAL_SESSION'"
run cargo run --quiet -- approvals list | (rg "$APPROVAL_ID" || true)
run cargo run --quiet -- approvals show "$APPROVAL_ID"
run cargo run --quiet -- approvals decide "$APPROVAL_ID" --decision denied --reason 'validation denial'
printf 'decision_status='; sql "select ar.status || ' decisions=' || count(ad.id) from approval_requests ar left join approval_decisions ad on ad.request_id=ar.id where ar.id='$APPROVAL_ID' group by ar.status"
printf 'decided_events='; sql "select count(*) from event_stream where session_id='$APPROVAL_SESSION' and event_type='approval.decided' and status='denied'"

run cargo run --quiet -- send --session "$DENY_SESSION" --message 'Use execute_code with exactly this Starlark source: fs.write("tmp/deny-routing.txt", "deny should not execute"); output("deny should not execute")'
printf '\n[deny no approval]\n'; sql "select jsonb_build_object('approvalRequests', count(*) filter (where event_type='approval.requested'), 'denies', count(*) filter (where event_type='policy.decision' and status='deny'), 'commands', count(*) filter (where event_type='command.completed')) from event_stream where session_id='$DENY_SESSION'"

run cargo run --quiet -- send --session "$ALLOW_SESSION" --message 'Use execute_code with exactly this Starlark source: text = fs.read("Cargo.toml"); matches = cmd["rg"].run(args=["--files", "-g", "Cargo.toml"], cwd=".").sync(); output("allow executes")'
printf '\n[allow executes]\n'; sql "select jsonb_build_object('allows', count(*) filter (where event_type='policy.decision' and status='allow'), 'commands', count(*) filter (where event_type='command.completed')) from event_stream where session_id='$ALLOW_SESSION'"
printf '\n[route decisions]\n'; sql "select jsonb_build_object('allowRoute', count(*) filter (where session_id='$ALLOW_SESSION' and event_type='route.decision'), 'denyRoute', count(*) filter (where session_id='$DENY_SESSION' and event_type='route.decision'), 'approvalRoute', count(*) filter (where session_id='$APPROVAL_SESSION' and event_type='route.decision')) from event_stream"

TMPDIR=$(mktemp -d /tmp/agent-runtime-routing.XXXXXX)
mkdir -p "$TMPDIR/prompts"
printf 'Dynamic target instructions.\n' > "$TMPDIR/prompts/dynamic-target.md"
printf 'Dynamic route instructions.\n' > "$TMPDIR/prompts/dynamic-route.md"
python3 - "$TMPDIR" <<'PY'
import json, pathlib, sys
root = pathlib.Path(sys.argv[1])
base = {
  "version":"1.0.0",
  "modelDefaults":{"model":"gpt-5.5","reasoningEffort":"medium"},
  "capabilities":["tool.execute_code","fs.read"],
  "policy":{"tool.execute_code":"allow","fs.read":"allow"},
  "visibility":{"listed":True,"ownerVisible":True},
  "lifecycleAuthority":{"canSpawnAgents":False,"canArchiveAgents":False,"reservedActions":["agent.spawn.<role>","agent.archive"]}
}
target = dict(base, id="runtime-dynamic-target", displayName="Runtime Dynamic Target", prompt={"path":"prompts/dynamic-target.md"}, routing={"mode":"direct","defaultRecipient":"owner","allowedRecipients":["owner"],"reservedActions":["message.send","message.route"]})
route = dict(base, id="runtime-dynamic-route", displayName="Runtime Dynamic Route", prompt={"path":"prompts/dynamic-route.md"}, routing={"mode":"direct","defaultRecipient":"runtime-dynamic-target","allowedRecipients":["runtime-dynamic-target","owner"],"reservedActions":["message.send","message.route"]})
invalid = dict(base, id="runtime-invalid-route", displayName="Runtime Invalid Route", prompt={"path":"prompts/dynamic-route.md"}, routing={"mode":"direct","defaultRecipient":"missing-role-recipient","allowedRecipients":["missing-role-recipient"],"reservedActions":["message.send","message.route"]})
(root/'dynamic-target.json').write_text(json.dumps(target, indent=2)+'\n')
(root/'dynamic-route.json').write_text(json.dumps(route, indent=2)+'\n')
(root/'invalid-route.json').write_text(json.dumps(invalid, indent=2)+'\n')
PY
run cargo run --quiet -- roles import "$TMPDIR/dynamic-target.json"
run cargo run --quiet -- roles import "$TMPDIR/dynamic-route.json"
printf 'dynamic_route_imported='; cargo run --quiet -- roles show runtime-dynamic-route | python3 -c 'import json,sys; d=json.load(sys.stdin); print(d["routing"]["defaultRecipient"]+" allowed="+",".join(d["routing"]["allowedRecipients"]))'
set +e
INVALID_OUTPUT=$(cargo run --quiet -- roles import "$TMPDIR/invalid-route.json" 2>&1)
INVALID_STATUS=$?
set -e
printf 'invalid_route_status=%s\n' "$INVALID_STATUS"
printf '%s\n' "$INVALID_OUTPUT" | rg 'invalid routing recipient: missing-role-recipient'
rm -rf "$TMPDIR"

printf '\n[validation complete]\n'

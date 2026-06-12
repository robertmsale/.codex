#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
source scripts/lib-validation-db.sh
validation_setup_database
run() { printf '\n$ %s\n' "$*"; "$@"; }
sql() { psql "$ROBDEX_AGENT_RUNTIME_DATABASE_URL" -At -c "$1"; }
run cargo run --quiet -- init-db
run cargo run --quiet -- roles import-seeds
ADMIN_SESSION=$(cargo run --quiet -- new-session --role runtime-allow)
printf 'admin_session=%s\n' "$ADMIN_SESSION"
printf '\n[seeded registry]\n'; cargo run --quiet -- command-registry list | tee /tmp/agent-runtime-command-registry-list.json
sql "select action_id || ':' || (current_version_id is not null) from command_definitions order by action_id"
printf 'seed_version_trace_columns='; sql "select count(*) from command_versions where action_id in ('cmd.rg.run','cmd.git.status','cmd.git.diff','cmd.cargo.check')"
ALLOW=$(cargo run --quiet -- new-session --role runtime-allow)
printf 'allow_session=%s\n' "$ALLOW"
run cargo run --quiet -- --workdir . send --session "$ALLOW" --message 'Use execute_code with exactly this Starlark source: matches = cmd["rg"].run(args=["--files", "-g", "Cargo.toml"], cwd="."); output(matches)'
printf 'command_version_id_count='; sql "select count(*) from command_runs where command_version_id is not null"
[[ "$(sql "select count(*) from command_runs where command_version_id is not null")" -gt 0 ]]
printf 'command_event_versions='; sql "select count(*) from event_stream where session_id='$ALLOW' and event_type='command.completed' and payload ? 'commandVersionId'"
[[ "$(sql "select count(*) from event_stream where session_id='$ALLOW' and event_type='command.completed' and payload ? 'commandVersionId'")" -gt 0 ]]
cat > /tmp/agent-runtime-command-request.json <<'JSON'
{"operation":"add","rationale":"validation adds a second rg surface","recommendedPolicy":"allow for validation role only","requester":"validation-script","command":{"actionId":"cmd.rg.files","binaryName":"rg","candidatePaths":["/opt/homebrew/bin/rg","/usr/local/bin/rg","/usr/bin/rg"],"starlarkObject":"rg_files","starlarkMethod":"run","argvPrefix":["--files"],"allowArgsArg":true,"allowCwdArg":true,"defaultCwd":".","cwdPolicy":"underExecutionRoot","envPolicy":"empty","timeoutMs":5000,"outputLimitBytes":12000,"mutationClass":"readOnly","modelDescription":"validation-only rg --files helper"}}
JSON
REQ_ID=$(cargo run --quiet -- command-registry requests create --session "$ADMIN_SESSION" /tmp/agent-runtime-command-request.json)
printf 'request_id=%s\n' "$REQ_ID"
APPLY_BEFORE=$(cargo run --quiet -- command-registry requests apply --session "$ADMIN_SESSION" "$REQ_ID" 2>&1 || true)
printf 'apply_before_approval=%s\n' "$APPLY_BEFORE" | rg 'must be approved'
run cargo run --quiet -- command-registry requests decide --session "$ADMIN_SESSION" "$REQ_ID" --status denied
DENIED_APPLY=$(cargo run --quiet -- command-registry requests apply --session "$ADMIN_SESSION" "$REQ_ID" 2>&1 || true)
printf 'denied_apply=%s\n' "$DENIED_APPLY" | rg 'must be approved'
printf 'denied_registry_count='; sql "select count(*) from command_definitions where action_id='cmd.rg.files'"
[[ "$(sql "select count(*) from command_definitions where action_id='cmd.rg.files'")" -eq 0 ]]
REQ_ID2=$(cargo run --quiet -- command-registry requests create --session "$ADMIN_SESSION" /tmp/agent-runtime-command-request.json)
run cargo run --quiet -- command-registry requests decide --session "$ADMIN_SESSION" "$REQ_ID2" --status approved
run cargo run --quiet -- command-registry requests apply --session "$ADMIN_SESSION" "$REQ_ID2"
printf 'approved_registry_count='; sql "select count(*) from command_definitions where action_id='cmd.rg.files'"
[[ "$(sql "select count(*) from command_definitions where action_id='cmd.rg.files'")" -eq 1 ]]

REQ_ID3=$(cargo run --quiet -- command-registry requests create --session "$ADMIN_SESSION" /tmp/agent-runtime-command-request.json)
run cargo run --quiet -- command-registry requests decide --session "$ADMIN_SESSION" "$REQ_ID3" --status approved
python3 - <<'PY2'
import json
from pathlib import Path
d=json.load(open('roles/runtime-allow.json'))
d['prompt']['path']=str(Path('roles/prompts/runtime-allow.md').resolve())
d['policy']['command_registry.apply']='ownerApproval'
open('/tmp/runtime-registry-apply-approval.json','w').write(json.dumps(d))
PY2
run cargo run --quiet -- roles import /tmp/runtime-registry-apply-approval.json
APPLY_APPROVAL_SESSION=$(cargo run --quiet -- new-session --role runtime-allow)
APPLY_APPROVAL_OUT=$(cargo run --quiet -- command-registry requests apply --session "$APPLY_APPROVAL_SESSION" "$REQ_ID3" 2>&1 || true)
printf 'registry_apply_approval=%s\n' "$APPLY_APPROVAL_OUT" | rg 'requires approval'
printf 'registry_apply_approval_events='; sql "select count(*) from approval_requests where session_id='$APPLY_APPROVAL_SESSION' and action_name='command_registry.apply'"
[[ "$(sql "select count(*) from approval_requests where session_id='$APPLY_APPROVAL_SESSION' and action_name='command_registry.apply'")" -gt 0 ]]
printf 'registry_apply_not_applied='; sql "select application_status from command_registry_requests where id='$REQ_ID3'"
[[ "$(sql "select application_status from command_registry_requests where id='$REQ_ID3'")" == "pending" ]]
REGISTRY_APPLY_APPROVAL_ID=$(sql "select approval_request_id from command_registry_requests where id='$REQ_ID3'")
printf 'registry_apply_approval_id=%s\n' "$REGISTRY_APPLY_APPROVAL_ID"
run cargo run --quiet -- approvals decide "$REGISTRY_APPLY_APPROVAL_ID" --decision approved --reason "registry apply validation approval"
run cargo run --quiet -- command-registry requests apply --session "$APPLY_APPROVAL_SESSION" "$REQ_ID3"
printf 'registry_apply_after_approval='; sql "select application_status from command_registry_requests where id='$REQ_ID3'"
[[ "$(sql "select application_status from command_registry_requests where id='$REQ_ID3'")" == "applied" ]]
python3 - <<'PY'
import json
p='roles/runtime-allow.json'
d=json.load(open(p))
d['prompt']['path']=str(__import__('pathlib').Path('roles/prompts/runtime-allow.md').resolve())
if 'cmd.rg.files' not in d['capabilities']:
    d['capabilities'].append('cmd.rg.files')
d['policy']['cmd.rg.files']='allow'
open('/tmp/runtime-allow-rg-files.json','w').write(json.dumps(d))
PY
run cargo run --quiet -- roles import /tmp/runtime-allow-rg-files.json
LIVE=$(cargo run --quiet -- new-session --role runtime-allow)
run cargo run --quiet -- --workdir . send --session "$LIVE" --message 'Use execute_code with exactly this Starlark source: files = cmd["rg_files"].run(args=["-g", "Cargo.toml"], cwd="."); output(files)'
printf 'live_new_command_runs='; sql "select count(*) from command_runs cr join command_versions cv on cv.id=cr.command_version_id where cv.action_id='cmd.rg.files'"
[[ "$(sql "select count(*) from command_runs cr join command_versions cv on cv.id=cr.command_version_id where cv.action_id='cmd.rg.files'")" -gt 0 ]]
python3 - <<'PY'
import json
p='roles/runtime-allow.json'
d=json.load(open(p))
d['prompt']['path']=str(__import__('pathlib').Path('roles/prompts/runtime-allow.md').resolve())
if 'cmd.rg.files' not in d['capabilities']:
    d['capabilities'].append('cmd.rg.files')
d['policy']['cmd.rg.files']='deny'
open('/tmp/runtime-deny-rg-files.json','w').write(json.dumps(d))
d['policy']['cmd.rg.files']='ownerApproval'
open('/tmp/runtime-approval-rg-files.json','w').write(json.dumps(d))
PY
run cargo run --quiet -- roles import /tmp/runtime-deny-rg-files.json
DENY_SESSION=$(cargo run --quiet -- new-session --role runtime-allow)
run cargo run --quiet -- --workdir . send --session "$DENY_SESSION" --message 'Use execute_code with exactly this Starlark source: files = cmd["rg_files"].run(args=["-g", "Cargo.toml"], cwd="."); output(files)'
printf 'db_action_deny_events='; sql "select count(*) from event_stream where session_id='$DENY_SESSION' and event_type='policy.decision' and status='deny' and payload->>'action'='cmd.rg.files'"
[[ "$(sql "select count(*) from event_stream where session_id='$DENY_SESSION' and event_type='policy.decision' and status='deny' and payload->>'action'='cmd.rg.files'")" -gt 0 ]]
run cargo run --quiet -- roles import /tmp/runtime-approval-rg-files.json
APPROVAL_SESSION=$(cargo run --quiet -- new-session --role runtime-allow)
run cargo run --quiet -- --workdir . send --session "$APPROVAL_SESSION" --message 'Use execute_code with exactly this Starlark source: files = cmd["rg_files"].run(args=["-g", "Cargo.toml"], cwd="."); output(files)'
printf 'db_action_approval_events='; sql "select count(*) from event_stream where session_id='$APPROVAL_SESSION' and event_type='policy.decision' and status='approvalRequired' and payload->>'action'='cmd.rg.files'"
[[ "$(sql "select count(*) from event_stream where session_id='$APPROVAL_SESSION' and event_type='policy.decision' and status='approvalRequired' and payload->>'action'='cmd.rg.files'")" -gt 0 ]]
run cargo run --quiet -- command-registry show cmd.rg.files

#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/lib-validation-db.sh"
validation_setup_database
run() { printf '\n$ %s\n' "$*"; "$@"; }
sql() { psql "$ROBDEX_AGENT_RUNTIME_DATABASE_URL" -Atc "$1"; }
ROOT="$(mktemp -d /tmp/robdex-agent-runtime-mutation.XXXXXX)"
cleanup_root() { rm -rf "$ROOT"; }
trap cleanup_root RETURN
cp Cargo.toml "$ROOT/Cargo.toml"
cat > "$ROOT/patch-target.txt" <<'TXT'
alpha
beta
gamma
TXT
cat > "$ROOT/patch-denied.txt" <<'TXT'
one
two
TXT
cat > "$ROOT/patch-approval.txt" <<'TXT'
red
blue
TXT
cat > "$ROOT/patch-fail.txt" <<'TXT'
left
right
TXT

run cargo run --quiet --bin robdex-agent-runtime -- init-db
run cargo run --quiet --bin robdex-agent-runtime -- roles import-seeds
ALLOW=$(cargo run --quiet --bin robdex-agent-runtime -- sessions new --role runtime-allow --workdir "$ROOT")
DENY=$(cargo run --quiet --bin robdex-agent-runtime -- sessions new --role runtime-no-rg --workdir "$ROOT")
APPROVE=$(cargo run --quiet --bin robdex-agent-runtime -- sessions new --role runtime-approval-rg --workdir "$ROOT")
printf '\n[sessions]\nALLOW=%s\nDENY=%s\nAPPROVE=%s\nROOT=%s\n' "$ALLOW" "$DENY" "$APPROVE" "$ROOT"

run cargo run --quiet --bin robdex-agent-runtime -- send --session "$ALLOW" --message 'Use execute_code with exactly this Starlark source: fs.write("notes.txt", "hello mutation"); output("wrote")'
printf 'fs_write_allowed='; sql "select jsonb_build_object('fileMutations', count(*), 'exists', (select count(*) from file_mutations where path like '%notes.txt' and status='completed')) from file_mutations"

set +e
OUTSIDE=$(cargo run --quiet --bin robdex-agent-runtime -- send --session "$ALLOW" --message 'Use execute_code with exactly this Starlark source: fs.write("../outside.txt", "bad"); output("bad")' 2>&1)
OUTSIDE_STATUS=$?
set -e
printf 'outside_root_status=%s\n' "$OUTSIDE_STATUS"
printf '%s\n' "$OUTSIDE"
printf 'outside_root_mutations='; sql "select count(*) from file_mutations where path like '%outside.txt' and status='completed'"

mkdir -p "$ROOT/.git"
set +e
GITWRITE=$(cargo run --quiet --bin robdex-agent-runtime -- send --session "$ALLOW" --message 'Use execute_code with exactly this Starlark source: fs.write(".git/config", "bad"); output("bad")' 2>&1)
GITWRITE_STATUS=$?
set -e
printf 'git_write_status=%s\n' "$GITWRITE_STATUS"
printf '%s\n' "$GITWRITE"
printf 'git_write_mutations='; sql "select count(*) from file_mutations where path like '%.git/config' and status='completed'"

run cargo run --quiet --bin robdex-agent-runtime -- send --session "$DENY" --message 'Use execute_code with exactly this Starlark source: fs.write("denied.txt", "nope"); output("denied")'
printf 'denied_write_counts='; sql "select jsonb_build_object('denies', count(*) filter (where event_type='policy.decision' and status='deny'), 'mutations', (select count(*) from file_mutations where path like '%denied.txt')) from event_stream where session_id='$DENY'"

run cargo run --quiet --bin robdex-agent-runtime -- send --session "$APPROVE" --message 'Use execute_code with exactly this Starlark source: fs.write("approved.txt", "stored content"); output("paused")'
APPROVAL_ID=$(sql "select id from approval_requests where session_id='$APPROVE' and action_name='fs.write' order by created_at desc limit 1")
printf 'approval_id=%s\n' "$APPROVAL_ID"
printf 'pre_resume_file_exists='; test -f "$ROOT/approved.txt" && echo yes || echo no
run cargo run --quiet --bin robdex-agent-runtime -- approvals decide "$APPROVAL_ID" --decision approved --reason 'mutation validation approval'
printf 'after_decide_file_exists='; test -f "$ROOT/approved.txt" && echo yes || echo no
run cargo run --quiet --bin robdex-agent-runtime -- approvals resume "$APPROVAL_ID"
printf 'after_resume_file_content='; cat "$ROOT/approved.txt"
printf '\nresume_file_mutations='; sql "select count(*) from file_mutations where path like '%approved.txt' and status='completed'"

PATCH='--- a/patch-target.txt
+++ b/patch-target.txt
@@ -1,3 +1,3 @@
 alpha
-beta
+delta
 gamma'
run cargo run --quiet --bin robdex-agent-runtime -- send --session "$ALLOW" --message "Use execute_code with exactly this Starlark source: patch.apply('''$PATCH'''); output('patched')"
printf 'patch_content='; cat "$ROOT/patch-target.txt"
printf '\npatch_runs='; sql "select count(*) from patch_runs where status='completed'"

DENY_PATCH='--- a/patch-denied.txt
+++ b/patch-denied.txt
@@ -1,2 +1,2 @@
 one
-two
+blocked'
run cargo run --quiet --bin robdex-agent-runtime -- send --session "$DENY" --message "Use execute_code with exactly this Starlark source: patch.apply('''$DENY_PATCH'''); output('denied patch')"
printf 'denied_patch_content='; cat "$ROOT/patch-denied.txt"
printf '\ndenied_patch_counts='; sql "select jsonb_build_object('denies', count(*) filter (where event_type='policy.decision' and status='deny'), 'patchRuns', (select count(*) from patch_runs where affected_paths::text like '%patch-denied.txt%')) from event_stream where session_id='$DENY'"

APPROVAL_PATCH='--- a/patch-approval.txt
+++ b/patch-approval.txt
@@ -1,2 +1,2 @@
 red
-blue
+green'
run cargo run --quiet --bin robdex-agent-runtime -- send --session "$APPROVE" --message "Use execute_code with exactly this Starlark source: patch.apply('''$APPROVAL_PATCH'''); output('approval patch')"
PATCH_APPROVAL_ID=$(sql "select id from approval_requests where session_id='$APPROVE' and action_name='patch.apply' order by created_at desc limit 1")
printf 'patch_approval_id=%s\n' "$PATCH_APPROVAL_ID"
printf 'pre_resume_patch_content='; cat "$ROOT/patch-approval.txt"
run cargo run --quiet --bin robdex-agent-runtime -- approvals decide "$PATCH_APPROVAL_ID" --decision approved --reason 'patch validation approval'
printf '\nafter_decide_patch_content='; cat "$ROOT/patch-approval.txt"
run cargo run --quiet --bin robdex-agent-runtime -- approvals resume "$PATCH_APPROVAL_ID"
printf '\nafter_resume_patch_content='; cat "$ROOT/patch-approval.txt"
printf '\nresumed_patch_runs='; sql "select count(*) from patch_runs where affected_paths::text like '%patch-approval.txt%' and status='completed'"

FAIL_PATCH='--- a/patch-fail.txt
+++ b/patch-fail.txt
@@ -1,2 +1,2 @@
 missing-context
-right
+wrong'
run cargo run --quiet --bin robdex-agent-runtime -- send --session "$APPROVE" --message "Use execute_code with exactly this Starlark source: patch.apply('''$FAIL_PATCH'''); output('approval fail patch')"
FAIL_PATCH_APPROVAL_ID=$(sql "select id from approval_requests where session_id='$APPROVE' and action_name='patch.apply' order by created_at desc limit 1")
run cargo run --quiet --bin robdex-agent-runtime -- approvals decide "$FAIL_PATCH_APPROVAL_ID" --decision approved --reason 'patch failure validation approval'
set +e
FAIL_RESUME_OUT=$(cargo run --quiet --bin robdex-agent-runtime -- approvals resume "$FAIL_PATCH_APPROVAL_ID" 2>&1)
FAIL_RESUME_STATUS=$?
set -e
printf '\nfailed_patch_resume_status=%s\n' "$FAIL_RESUME_STATUS"
printf '%s\n' "$FAIL_RESUME_OUT" | rg 'approval resume failed|patch context mismatch|patch removal mismatch'
printf 'failed_resumed_patch_runs='; sql "select jsonb_build_object('failedRows', count(*) filter (where status='failed'), 'failedEvents', (select count(*) from event_stream where event_type='patch.completed' and status='failed')) from patch_runs where affected_paths::text like '%patch-fail.txt%'"

BADPATCH='--- a/.git/config
+++ b/.git/config
@@ -1,1 +1,1 @@
-a
+b'
set +e
BADPATCH_OUT=$(cargo run --quiet --bin robdex-agent-runtime -- send --session "$ALLOW" --message "Use execute_code with exactly this Starlark source: patch.apply('''$BADPATCH'''); output('bad')" 2>&1)
BADPATCH_STATUS=$?
set -e
printf 'bad_patch_status=%s\n' "$BADPATCH_STATUS"
printf '%s\n' "$BADPATCH_OUT" | rg 'git internals|failed'

run cargo run --quiet --bin robdex-agent-runtime -- send --session "$ALLOW" --message 'Use execute_code with exactly this Starlark source: s = cmd["git"].status(); d = cmd["git"].diff(args=[]); output(s + d)'
printf 'git_command_runs='; sql "select jsonb_build_object('gitRuns', count(*), 'statusRuns', count(*) filter (where argv->>0='status'), 'diffRuns', count(*) filter (where argv->>0='diff')) from command_runs where binary_name='git'"
run cargo run --quiet --bin robdex-agent-runtime -- send --session "$ALLOW" --message 'Use execute_code with exactly this Starlark source: result = cmd["cargo"].check(args=[]); output("cargo checked")'
printf 'cargo_command_runs='; sql "select count(*) from command_runs where binary_name='cargo'"
printf 'ordered_events='; sql "select string_agg(event_type, ' > ' order by sequence) from event_stream where session_id in ('$ALLOW','$APPROVE') and event_type in ('policy.decision','approval.requested','approval.resume.started','policy.resumeDecision','file_mutation.completed','patch.completed','command.completed','approval.resume.completed')"
printf '\n[validation complete]\n'

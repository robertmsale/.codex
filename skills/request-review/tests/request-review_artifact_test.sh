#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
skill_dir="$(cd "$script_dir/.." && pwd)"

tmp_root="$(mktemp -d)"
trap 'rm -rf "$tmp_root"' EXIT

fail() {
  echo "FAIL: $*" >&2
  exit 1
}

assert_file_contains() {
  local path="$1"
  local expected="$2"

  [[ -f "$path" ]] || fail "expected file to exist: $path"
  local actual
  actual="$(cat "$path")"
  [[ "$actual" == "$expected" ]] || fail "expected '$expected' in $path, got '$actual'"
}

assert_file_missing() {
  local path="$1"
  [[ ! -e "$path" ]] || fail "expected file to be absent: $path"
}

setup_repo() {
  local repo_dir="$1"
  local origin_dir="$2"

  mkdir -p "$repo_dir"
  git init "$repo_dir" >/dev/null
  git -C "$repo_dir" config user.name "Codex Test"
  git -C "$repo_dir" config user.email "codex@example.com"
  printf 'base\n' >"$repo_dir/file.txt"
  git -C "$repo_dir" add file.txt
  git -C "$repo_dir" commit -m "base" >/dev/null
  git -C "$repo_dir" checkout -b feature/request-review-artifact >/dev/null

  git init --bare "$origin_dir" >/dev/null
  git -C "$repo_dir" remote add origin "$origin_dir"
}

setup_common_home() {
  local home_dir="$1"
  mkdir -p "$home_dir/.codex/tmp" "$home_dir/.codex/scripts"
}

setup_common_stubs() {
  local bin_dir="$1"

  mkdir -p "$bin_dir"

  cat >"$bin_dir/codex" <<'EOF'
#!/usr/bin/env bash
exit 0
EOF
  chmod +x "$bin_dir/codex"
}

setup_local_review_stubs() {
  local home_dir="$1"
  local bin_dir="$2"

  cat >"$home_dir/.codex/config.toml" <<'EOF'
[profiles.local-review]
model = "test"
EOF

  cat >"$home_dir/.codex/scripts/build-codex-agent-image" <<'EOF'
#!/usr/bin/env bash
exit 0
EOF
  chmod +x "$home_dir/.codex/scripts/build-codex-agent-image"

  cat >"$bin_dir/docker" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

if [[ "${1:-}" == "image" && "${2:-}" == "inspect" ]]; then
  exit 0
fi

if [[ "${1:-}" == "run" ]]; then
  printf '%s\n' '{"type":"item.completed","item":{"type":"agent_message","text":"needs changes"}}'
  exit 1
fi

exit 2
EOF
  chmod +x "$bin_dir/docker"
}

make_skill_copy() {
  local dest_dir="$1"
  local env_contents="$2"

  cp -R "$skill_dir" "$dest_dir"
  printf '%s\n' "$env_contents" >"$dest_dir/.env"
}

test_remote_disable_writes_review_log() {
  local repo_dir="$tmp_root/remote-repo"
  local origin_dir="$tmp_root/remote-origin.git"
  local home_dir="$tmp_root/remote-home"
  local bin_dir="$tmp_root/remote-bin"
  local skill_copy_dir="$tmp_root/remote-skill"
  local request_review_script="$skill_copy_dir/scripts/request-review"
  local output

  setup_repo "$repo_dir" "$origin_dir"
  setup_common_home "$home_dir"
  setup_common_stubs "$bin_dir"
  make_skill_copy "$skill_copy_dir" $'REQUEST_REVIEW_MODE=remote\nREQUEST_REVIEW_DISABLE=1'

  printf 'stale\n' >"$repo_dir/review.log"
  printf 'remote change\n' >>"$repo_dir/file.txt"

  output="$(
    cd "$repo_dir" &&
      HOME="$home_dir" \
      PATH="$bin_dir:$PATH" \
      "$request_review_script" "test: remote disable writes artifact"
  )"

  [[ "$output" == *"all clear!"* ]] || fail "unexpected remote disable output: $output"
  assert_file_contains "$repo_dir/review.log" "all clear!"
  git --git-dir="$origin_dir" rev-parse --verify refs/heads/feature/request-review-artifact >/dev/null
}

test_local_failure_clears_review_log() {
  local repo_dir="$tmp_root/local-repo"
  local origin_dir="$tmp_root/local-origin.git"
  local home_dir="$tmp_root/local-home"
  local bin_dir="$tmp_root/local-bin"
  local skill_copy_dir="$tmp_root/local-skill"
  local request_review_script="$skill_copy_dir/scripts/request-review"
  local output
  local status

  setup_repo "$repo_dir" "$origin_dir"
  setup_common_home "$home_dir"
  setup_common_stubs "$bin_dir"
  setup_local_review_stubs "$home_dir" "$bin_dir"
  make_skill_copy "$skill_copy_dir" 'REQUEST_REVIEW_MODE=local'

  printf 'stale\n' >"$repo_dir/review.log"
  printf 'local change\n' >>"$repo_dir/file.txt"

  set +e
  output="$(
    cd "$repo_dir" &&
      HOME="$home_dir" \
      PATH="$bin_dir:$PATH" \
      "$request_review_script" "test: local failure clears artifact" 2>&1
  )"
  status=$?
  set -e

  [[ $status -ne 0 ]] || fail "expected local review failure"
  [[ "$output" == *"needs changes"* ]] || fail "expected local review summary in output"
  assert_file_missing "$repo_dir/review.log"
}

test_remote_disable_writes_review_log
test_local_failure_clears_review_log

echo "PASS: request-review artifact handling"

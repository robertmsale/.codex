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

assert_file_includes() {
  local path="$1"
  local expected_fragment="$2"

  [[ -f "$path" ]] || fail "expected file to exist: $path"
  local actual
  actual="$(cat "$path")"
  [[ "$actual" == *"$expected_fragment"* ]] || fail "expected '$expected_fragment' in $path, got '$actual'"
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

setup_remote_review_stubs() {
  local bin_dir="$1"

  cat >"$bin_dir/gh" <<EOF
#!/usr/bin/env bash
set -euo pipefail

if [[ "\${1:-}" == "repo" && "\${2:-}" == "view" ]]; then
  if [[ "\$*" == *".defaultBranchRef.name"* ]]; then
    echo "\${REQUEST_REVIEW_TEST_DEFAULT_BRANCH:-main}"
    exit 0
  fi
  echo "example/repo"
  exit 0
fi

if [[ "\${1:-}" == "pr" && "\${2:-}" == "view" ]]; then
  if [[ -n "\${REQUEST_REVIEW_TEST_PR_EXISTS_FILE:-}" && ! -f "\${REQUEST_REVIEW_TEST_PR_EXISTS_FILE}" ]]; then
    exit 1
  fi
  if [[ "\$*" == *".number"* ]]; then
    echo "711"
    exit 0
  fi
  if [[ "\$*" == *".state"* ]]; then
    echo "OPEN"
    exit 0
  fi
fi

if [[ "\${1:-}" == "pr" && "\${2:-}" == "comment" ]]; then
  exit 0
fi

if [[ "\${1:-}" == "pr" && "\${2:-}" == "create" ]]; then
  if [[ -n "\${REQUEST_REVIEW_TEST_PR_EXISTS_FILE:-}" ]]; then
    : >"\${REQUEST_REVIEW_TEST_PR_EXISTS_FILE}"
  fi
  echo "https://github.com/example/repo/pull/711"
  exit 0
fi

if [[ "\${1:-}" == "api" && "\${2:-}" == "--method" && "\${3:-}" == "POST" && "\${4:-}" == *"/issues/711/comments"* ]]; then
  echo "\${REQUEST_REVIEW_EXPECTED_TRIGGER_TIME:-2026-03-08T00:00:00Z}"
  exit 0
fi

if [[ "\${1:-}" == "api" && "\${2:-}" == *"/pulls/711/commits"* ]]; then
  echo "2026-03-07T00:00:00Z"
  exit 0
fi

if [[ "\${1:-}" == "api" && "\${2:-}" == *"/pulls/711/comments"* ]]; then
  echo "[]"
  exit 0
fi

if [[ "\${1:-}" == "api" && "\${2:-}" == *"/issues/711/comments"* ]]; then
  if [[ -n "\${REQUEST_REVIEW_EXPECTED_TRIGGER_TIME:-}" && ( "\$*" != *"\${REQUEST_REVIEW_EXPECTED_TRIGGER_TIME}"* || "\$*" != *".created_at >="* ) ]]; then
    echo "missing inclusive trigger-time filter in issue comments query: \$*" >&2
    exit 1
  fi
  echo "\${REQUEST_REVIEW_TEST_ISSUE_COMMENTS_JSON:-[]}"
  exit 0
fi

if [[ "\${1:-}" == "api" && "\${2:-}" == *"/issues/711/reactions"* ]]; then
  if [[ -n "\${REQUEST_REVIEW_EXPECTED_TRIGGER_TIME:-}" && ( "\$*" != *"\${REQUEST_REVIEW_EXPECTED_TRIGGER_TIME}"* || "\$*" != *".created_at >="* ) ]]; then
    echo "missing inclusive trigger-time filter in reactions query: \$*" >&2
    exit 1
  fi
  echo '[{"content":"+1","created_at":"2026-03-08T00:00:01Z"}]'
  exit 0
fi

echo "unexpected gh invocation: \$*" >&2
exit 1
EOF
  chmod +x "$bin_dir/gh"
}

setup_local_success_review_stubs() {
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
  printf '%s\n' '{"type":"item.completed","item":{"type":"agent_message","text":"all clear!"}}'
  exit 0
fi

exit 2
EOF
  chmod +x "$bin_dir/docker"
}

setup_local_capture_review_stubs() {
  local home_dir="$1"
  local bin_dir="$2"
  local capture_path="$3"

  cat >"$home_dir/.codex/config.toml" <<'EOF'
[profiles.local-review]
model = "test"
EOF

  cat >"$home_dir/.codex/scripts/build-codex-agent-image" <<'EOF'
#!/usr/bin/env bash
exit 0
EOF
  chmod +x "$home_dir/.codex/scripts/build-codex-agent-image"

  cat >"$bin_dir/docker" <<EOF
#!/usr/bin/env bash
set -euo pipefail

if [[ "\${1:-}" == "image" && "\${2:-}" == "inspect" ]]; then
  exit 0
fi

if [[ "\${1:-}" == "run" ]]; then
  printf '%s\n' "\$@" >"$capture_path"
  printf '%s\n' '{"type":"item.completed","item":{"type":"agent_message","text":"all clear!"}}'
  exit 0
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

test_canonical_env_overrides_agent_runtime_env() {
  local repo_dir="$tmp_root/canonical-env-repo"
  local origin_dir="$tmp_root/canonical-env-origin.git"
  local home_dir="$tmp_root/canonical-env-home"
  local bin_dir="$tmp_root/canonical-env-bin"
  local skill_copy_dir="$tmp_root/canonical-env-skill"
  local request_review_script="$skill_copy_dir/scripts/request-review"
  local output
  local canonical_env_dir="$home_dir/.codex/skills/request-review"

  setup_repo "$repo_dir" "$origin_dir"
  setup_common_home "$home_dir"
  setup_common_stubs "$bin_dir"
  setup_local_success_review_stubs "$home_dir" "$bin_dir"
  mkdir -p "$canonical_env_dir"
  printf '%s\n' 'REQUEST_REVIEW_MODE=local' >"$canonical_env_dir/.env"
  cp -R "$skill_dir" "$skill_copy_dir"
  rm -f "$skill_copy_dir/.env"

  output="$(
    cd "$repo_dir" &&
      HOME="$home_dir" \
      CODEX_HOME="$home_dir/.codex" \
      PATH="$bin_dir:$PATH" \
      REQUEST_REVIEW_MODE=remote \
      "$request_review_script" --use-existing-commit "test: canonical env wins"
  )"

  [[ "$output" == *"all clear!"* ]] || fail "expected canonical env to force local mode, got: $output"
  assert_file_contains "$repo_dir/review.log" "all clear!"
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

test_remote_rerun_reuses_head_when_pr_is_open() {
  local repo_dir="$tmp_root/remote-rerun-repo"
  local origin_dir="$tmp_root/remote-rerun-origin.git"
  local home_dir="$tmp_root/remote-rerun-home"
  local bin_dir="$tmp_root/remote-rerun-bin"
  local skill_copy_dir="$tmp_root/remote-rerun-skill"
  local request_review_script="$skill_copy_dir/scripts/request-review"
  local output
  local review_sha
  local expected
  local pr_exists_file="$tmp_root/remote-rerun-pr-open"

  setup_repo "$repo_dir" "$origin_dir"
  setup_common_home "$home_dir"
  setup_common_stubs "$bin_dir"

  printf 'remote rerun change\n' >>"$repo_dir/file.txt"
  git -C "$repo_dir" add file.txt
  git -C "$repo_dir" commit -m "existing remote commit" >/dev/null
  git -C "$repo_dir" push origin feature/request-review-artifact >/dev/null
  review_sha="$(git -C "$repo_dir" rev-parse HEAD)"

  setup_remote_review_stubs "$bin_dir"
  make_skill_copy "$skill_copy_dir" 'REQUEST_REVIEW_MODE=remote'
  : >"$pr_exists_file"

  output="$(
    cd "$repo_dir" &&
      HOME="$home_dir" \
      PATH="$bin_dir:$PATH" \
      REQUEST_REVIEW_TEST_PR_EXISTS_FILE="$pr_exists_file" \
      REQUEST_REVIEW_EXPECTED_TRIGGER_TIME=2026-03-08T00:00:00Z \
      "$request_review_script" "test: remote rerun reuses head"
  )"

  expected="request-review (remote): 👍 from chatgpt-codex-connector[bot] on PR #711 after commit $review_sha"
  [[ "$output" == *"$expected"* ]] || fail "unexpected remote rerun output: $output"
  assert_file_contains "$repo_dir/review.log" "$expected"
}

test_remote_rerun_pushes_head_when_pr_branch_is_behind() {
  local repo_dir="$tmp_root/remote-rerun-push-repo"
  local origin_dir="$tmp_root/remote-rerun-push-origin.git"
  local home_dir="$tmp_root/remote-rerun-push-home"
  local bin_dir="$tmp_root/remote-rerun-push-bin"
  local skill_copy_dir="$tmp_root/remote-rerun-push-skill"
  local request_review_script="$skill_copy_dir/scripts/request-review"
  local output
  local review_sha
  local expected
  local remote_sha
  local pr_exists_file="$tmp_root/remote-rerun-push-pr-open"

  setup_repo "$repo_dir" "$origin_dir"
  setup_common_home "$home_dir"
  setup_common_stubs "$bin_dir"

  printf 'remote rerun pushed base\n' >>"$repo_dir/file.txt"
  git -C "$repo_dir" add file.txt
  git -C "$repo_dir" commit -m "pushed remote commit" >/dev/null
  git -C "$repo_dir" push origin feature/request-review-artifact >/dev/null

  printf 'remote rerun local only\n' >>"$repo_dir/file.txt"
  git -C "$repo_dir" add file.txt
  git -C "$repo_dir" commit -m "local only commit" >/dev/null
  review_sha="$(git -C "$repo_dir" rev-parse HEAD)"

  setup_remote_review_stubs "$bin_dir"
  make_skill_copy "$skill_copy_dir" 'REQUEST_REVIEW_MODE=remote'
  : >"$pr_exists_file"

  output="$(
    cd "$repo_dir" &&
      HOME="$home_dir" \
      PATH="$bin_dir:$PATH" \
      REQUEST_REVIEW_TEST_PR_EXISTS_FILE="$pr_exists_file" \
      REQUEST_REVIEW_EXPECTED_TRIGGER_TIME=2026-03-08T00:00:00Z \
      "$request_review_script" "test: remote rerun pushes head when pr branch is behind"
  )"

  expected="request-review (remote): 👍 from chatgpt-codex-connector[bot] on PR #711 after commit $review_sha"
  [[ "$output" == *"$expected"* ]] || fail "unexpected remote rerun push output: $output"
  remote_sha="$(git --git-dir="$origin_dir" rev-parse refs/heads/feature/request-review-artifact)"
  [[ "$remote_sha" == "$review_sha" ]] || fail "expected remote branch to advance to $review_sha, got $remote_sha"
}

test_remote_terminal_issue_comment_exits_nonzero() {
  local repo_dir="$tmp_root/remote-terminal-comment-repo"
  local origin_dir="$tmp_root/remote-terminal-comment-origin.git"
  local home_dir="$tmp_root/remote-terminal-comment-home"
  local bin_dir="$tmp_root/remote-terminal-comment-bin"
  local skill_copy_dir="$tmp_root/remote-terminal-comment-skill"
  local request_review_script="$skill_copy_dir/scripts/request-review"
  local output
  local status
  local review_sha
  local expected
  local pr_exists_file="$tmp_root/remote-terminal-comment-pr-open"
  local issue_comments_json

  setup_repo "$repo_dir" "$origin_dir"
  setup_common_home "$home_dir"
  setup_common_stubs "$bin_dir"
  setup_remote_review_stubs "$bin_dir"
  make_skill_copy "$skill_copy_dir" 'REQUEST_REVIEW_MODE=remote'

  printf 'remote terminal comment change\n' >>"$repo_dir/file.txt"
  git -C "$repo_dir" add file.txt
  git -C "$repo_dir" commit -m "remote terminal comment commit" >/dev/null
  git -C "$repo_dir" push origin feature/request-review-artifact >/dev/null
  review_sha="$(git -C "$repo_dir" rev-parse HEAD)"
  : >"$pr_exists_file"

  issue_comments_json="$(cat <<'JSON'
[{"created_at":"2026-03-08T00:00:05Z","html_url":"https://github.com/example/repo/pull/711#issuecomment-1","body":"Codex Review: Something went wrong. Try again later by commenting \"@codex review\"."}]
JSON
)"

  set +e
  output="$(
    cd "$repo_dir" &&
      HOME="$home_dir" \
      PATH="$bin_dir:$PATH" \
      REQUEST_REVIEW_TEST_PR_EXISTS_FILE="$pr_exists_file" \
      REQUEST_REVIEW_EXPECTED_TRIGGER_TIME=2026-03-08T00:00:00Z \
      REQUEST_REVIEW_TEST_ISSUE_COMMENTS_JSON="$issue_comments_json" \
      "$request_review_script" "test: remote terminal issue comment" 2>&1
  )"
  status=$?
  set -e

  [[ $status -ne 0 ]] || fail "expected remote terminal bot comment to exit nonzero"
  expected=$'request-review (remote): terminal bot error comment from chatgpt-codex-connector[bot] on PR #711 after commit '"$review_sha"$'\n\n- https://github.com/example/repo/pull/711#issuecomment-1\n  Comment: Codex Review: Something went wrong. Try again later by commenting "@codex review".'
  [[ "$output" == *"$expected"* ]] || fail "unexpected terminal-comment output: $output"
  assert_file_contains "$repo_dir/review.log" "$expected"
}

test_use_existing_commit_flag_reviews_clean_head() {
  local repo_dir="$tmp_root/existing-commit-repo"
  local origin_dir="$tmp_root/existing-commit-origin.git"
  local home_dir="$tmp_root/existing-commit-home"
  local bin_dir="$tmp_root/existing-commit-bin"
  local skill_copy_dir="$tmp_root/existing-commit-skill"
  local request_review_script="$skill_copy_dir/scripts/request-review"
  local output
  local review_sha

  setup_repo "$repo_dir" "$origin_dir"
  setup_common_home "$home_dir"
  setup_common_stubs "$bin_dir"
  setup_local_success_review_stubs "$home_dir" "$bin_dir"
  make_skill_copy "$skill_copy_dir" 'REQUEST_REVIEW_MODE=local'

  review_sha="$(git -C "$repo_dir" rev-parse HEAD)"

  output="$(
    cd "$repo_dir" &&
      HOME="$home_dir" \
      PATH="$bin_dir:$PATH" \
      "$request_review_script" --use-existing-commit "test: reuse existing head commit"
  )"

  [[ "$output" == *"all clear!"* ]] || fail "unexpected existing commit output: $output"
  assert_file_contains "$repo_dir/review.log" "all clear!"
  [[ "$(git -C "$repo_dir" rev-parse HEAD)" == "$review_sha" ]] || fail "expected HEAD to remain unchanged"
}

test_existing_commit_flag_like_text_stays_in_message() {
  local repo_dir="$tmp_root/existing-commit-message-repo"
  local origin_dir="$tmp_root/existing-commit-message-origin.git"
  local home_dir="$tmp_root/existing-commit-message-home"
  local bin_dir="$tmp_root/existing-commit-message-bin"
  local skill_copy_dir="$tmp_root/existing-commit-message-skill"
  local request_review_script="$skill_copy_dir/scripts/request-review"
  local output

  setup_repo "$repo_dir" "$origin_dir"
  setup_common_home "$home_dir"
  setup_common_stubs "$bin_dir"
  setup_local_success_review_stubs "$home_dir" "$bin_dir"
  make_skill_copy "$skill_copy_dir" 'REQUEST_REVIEW_MODE=local'

  printf 'real change\n' >>"$repo_dir/file.txt"

  output="$(
    cd "$repo_dir" &&
      HOME="$home_dir" \
      PATH="$bin_dir:$PATH" \
      "$request_review_script" fix parser --use-existing-commit text
  )"

  [[ "$output" == *"all clear!"* ]] || fail "unexpected flag-like message output: $output"
  assert_file_contains "$repo_dir/review.log" "all clear!"
  [[ "$(git -C "$repo_dir" log -1 --pretty=%s)" == "fix parser --use-existing-commit text" ]] || fail "expected full message to be committed verbatim"
}

test_remote_dirty_worktree_creates_pr_and_reviews_in_one_shot() {
  local repo_dir="$tmp_root/remote-dirty-repo"
  local origin_dir="$tmp_root/remote-dirty-origin.git"
  local home_dir="$tmp_root/remote-dirty-home"
  local bin_dir="$tmp_root/remote-dirty-bin"
  local skill_copy_dir="$tmp_root/remote-dirty-skill"
  local request_review_script="$skill_copy_dir/scripts/request-review"
  local output
  local review_sha
  local expected
  local pr_exists_file="$tmp_root/remote-dirty-pr-open"
  local remote_sha

  setup_repo "$repo_dir" "$origin_dir"
  setup_common_home "$home_dir"
  setup_common_stubs "$bin_dir"
  setup_remote_review_stubs "$bin_dir"
  make_skill_copy "$skill_copy_dir" 'REQUEST_REVIEW_MODE=remote'

  printf 'dirty remote change\n' >>"$repo_dir/file.txt"

  output="$(
    cd "$repo_dir" &&
      HOME="$home_dir" \
      PATH="$bin_dir:$PATH" \
      REQUEST_REVIEW_TEST_PR_EXISTS_FILE="$pr_exists_file" \
      REQUEST_REVIEW_EXPECTED_TRIGGER_TIME=2026-03-08T00:00:00Z \
      "$request_review_script" "test: remote dirty one shot"
  )"

  review_sha="$(git -C "$repo_dir" rev-parse HEAD)"
  expected="request-review (remote): 👍 from chatgpt-codex-connector[bot] on PR #711 after commit $review_sha"
  [[ "$output" == *"$expected"* ]] || fail "unexpected remote dirty output: $output"
  [[ "$(git -C "$repo_dir" log -1 --pretty=%s)" == "test: remote dirty one shot" ]] || fail "expected dirty path to create the intended commit"
  [[ -f "$pr_exists_file" ]] || fail "expected remote dirty path to create a PR"
  remote_sha="$(git --git-dir="$origin_dir" rev-parse refs/heads/feature/request-review-artifact)"
  [[ "$remote_sha" == "$review_sha" ]] || fail "expected remote branch to advance to $review_sha, got $remote_sha"
  assert_file_contains "$repo_dir/review.log" "$expected"
}

test_remote_clean_head_without_pr_reuses_head_and_creates_pr() {
  local repo_dir="$tmp_root/remote-clean-repo"
  local origin_dir="$tmp_root/remote-clean-origin.git"
  local home_dir="$tmp_root/remote-clean-home"
  local bin_dir="$tmp_root/remote-clean-bin"
  local skill_copy_dir="$tmp_root/remote-clean-skill"
  local request_review_script="$skill_copy_dir/scripts/request-review"
  local output
  local review_sha
  local expected
  local pr_exists_file="$tmp_root/remote-clean-pr-open"
  local remote_sha

  setup_repo "$repo_dir" "$origin_dir"
  setup_common_home "$home_dir"
  setup_common_stubs "$bin_dir"
  setup_remote_review_stubs "$bin_dir"
  make_skill_copy "$skill_copy_dir" 'REQUEST_REVIEW_MODE=remote'

  printf 'clean remote commit\n' >>"$repo_dir/file.txt"
  git -C "$repo_dir" add file.txt
  git -C "$repo_dir" commit -m "existing clean commit" >/dev/null
  review_sha="$(git -C "$repo_dir" rev-parse HEAD)"

  output="$(
    cd "$repo_dir" &&
      HOME="$home_dir" \
      PATH="$bin_dir:$PATH" \
      REQUEST_REVIEW_TEST_PR_EXISTS_FILE="$pr_exists_file" \
      REQUEST_REVIEW_EXPECTED_TRIGGER_TIME=2026-03-08T00:00:00Z \
      "$request_review_script" "test: remote clean one shot"
  )"

  expected="request-review (remote): 👍 from chatgpt-codex-connector[bot] on PR #711 after commit $review_sha"
  [[ "$output" == *"$expected"* ]] || fail "unexpected remote clean output: $output"
  [[ "$(git -C "$repo_dir" rev-parse HEAD)" == "$review_sha" ]] || fail "expected clean path to reuse existing HEAD"
  [[ "$(git -C "$repo_dir" log -1 --pretty=%s)" == "existing clean commit" ]] || fail "expected clean path not to create a new commit"
  [[ -f "$pr_exists_file" ]] || fail "expected remote clean path to create a PR"
  remote_sha="$(git --git-dir="$origin_dir" rev-parse refs/heads/feature/request-review-artifact)"
  [[ "$remote_sha" == "$review_sha" ]] || fail "expected remote branch to advance to $review_sha, got $remote_sha"
  assert_file_contains "$repo_dir/review.log" "$expected"
}

test_remote_existing_commit_pushes_selected_sha_not_head() {
  local repo_dir="$tmp_root/remote-existing-commit-repo"
  local origin_dir="$tmp_root/remote-existing-commit-origin.git"
  local home_dir="$tmp_root/remote-existing-commit-home"
  local bin_dir="$tmp_root/remote-existing-commit-bin"
  local skill_copy_dir="$tmp_root/remote-existing-commit-skill"
  local request_review_script="$skill_copy_dir/scripts/request-review"
  local output
  local review_sha
  local head_sha
  local expected
  local pr_exists_file="$tmp_root/remote-existing-commit-pr-open"
  local remote_sha

  setup_repo "$repo_dir" "$origin_dir"
  setup_common_home "$home_dir"
  setup_common_stubs "$bin_dir"
  setup_remote_review_stubs "$bin_dir"
  make_skill_copy "$skill_copy_dir" 'REQUEST_REVIEW_MODE=remote'

  printf 'review target commit\n' >>"$repo_dir/file.txt"
  git -C "$repo_dir" add file.txt
  git -C "$repo_dir" commit -m "review target commit" >/dev/null
  review_sha="$(git -C "$repo_dir" rev-parse HEAD)"

  printf 'newer unrelated commit\n' >>"$repo_dir/file.txt"
  git -C "$repo_dir" add file.txt
  git -C "$repo_dir" commit -m "newer unrelated commit" >/dev/null
  head_sha="$(git -C "$repo_dir" rev-parse HEAD)"

  output="$(
    cd "$repo_dir" &&
      HOME="$home_dir" \
      PATH="$bin_dir:$PATH" \
      REQUEST_REVIEW_TEST_PR_EXISTS_FILE="$pr_exists_file" \
      REQUEST_REVIEW_EXPECTED_TRIGGER_TIME=2026-03-08T00:00:00Z \
      "$request_review_script" --use-existing-commit --existing-commit "$review_sha" "test: remote existing commit"
  )"

  expected="request-review (remote): 👍 from chatgpt-codex-connector[bot] on PR #711 after commit $review_sha"
  [[ "$output" == *"$expected"* ]] || fail "unexpected remote existing-commit output: $output"
  [[ "$(git -C "$repo_dir" rev-parse HEAD)" == "$head_sha" ]] || fail "expected local HEAD to remain on the newer commit"
  remote_sha="$(git --git-dir="$origin_dir" rev-parse refs/heads/feature/request-review-artifact)"
  [[ "$remote_sha" == "$review_sha" ]] || fail "expected remote branch to point at requested review sha $review_sha, got $remote_sha"
  assert_file_contains "$repo_dir/review.log" "$expected"
}

test_remote_existing_ancestor_commit_skips_non_fast_forward_push() {
  local repo_dir="$tmp_root/remote-ancestor-commit-repo"
  local origin_dir="$tmp_root/remote-ancestor-commit-origin.git"
  local home_dir="$tmp_root/remote-ancestor-commit-home"
  local bin_dir="$tmp_root/remote-ancestor-commit-bin"
  local skill_copy_dir="$tmp_root/remote-ancestor-commit-skill"
  local request_review_script="$skill_copy_dir/scripts/request-review"
  local output
  local review_sha
  local head_sha
  local expected
  local pr_exists_file="$tmp_root/remote-ancestor-commit-pr-open"
  local remote_sha

  setup_repo "$repo_dir" "$origin_dir"
  setup_common_home "$home_dir"
  setup_common_stubs "$bin_dir"
  setup_remote_review_stubs "$bin_dir"
  make_skill_copy "$skill_copy_dir" 'REQUEST_REVIEW_MODE=remote'

  printf 'ancestor review target\n' >>"$repo_dir/file.txt"
  git -C "$repo_dir" add file.txt
  git -C "$repo_dir" commit -m "ancestor review target" >/dev/null
  review_sha="$(git -C "$repo_dir" rev-parse HEAD)"

  printf 'already pushed head\n' >>"$repo_dir/file.txt"
  git -C "$repo_dir" add file.txt
  git -C "$repo_dir" commit -m "already pushed head" >/dev/null
  head_sha="$(git -C "$repo_dir" rev-parse HEAD)"
  git -C "$repo_dir" push origin feature/request-review-artifact >/dev/null

  output="$(
    cd "$repo_dir" &&
      HOME="$home_dir" \
      PATH="$bin_dir:$PATH" \
      REQUEST_REVIEW_TEST_PR_EXISTS_FILE="$pr_exists_file" \
      REQUEST_REVIEW_EXPECTED_TRIGGER_TIME=2026-03-08T00:00:00Z \
      "$request_review_script" --use-existing-commit --existing-commit "$review_sha" "test: remote ancestor existing commit"
  )"

  expected="request-review (remote): 👍 from chatgpt-codex-connector[bot] on PR #711 after commit $review_sha"
  [[ "$output" == *"$expected"* ]] || fail "unexpected remote ancestor existing-commit output: $output"
  [[ "$(git -C "$repo_dir" rev-parse HEAD)" == "$head_sha" ]] || fail "expected local HEAD to remain on the newer pushed commit"
  remote_sha="$(git --git-dir="$origin_dir" rev-parse refs/heads/feature/request-review-artifact)"
  [[ "$remote_sha" == "$head_sha" ]] || fail "expected remote branch to stay on $head_sha, got $remote_sha"
  assert_file_contains "$repo_dir/review.log" "$expected"
}

test_remote_rewritten_head_force_pushes_with_lease() {
  local repo_dir="$tmp_root/remote-rewritten-head-repo"
  local origin_dir="$tmp_root/remote-rewritten-head-origin.git"
  local home_dir="$tmp_root/remote-rewritten-head-home"
  local bin_dir="$tmp_root/remote-rewritten-head-bin"
  local skill_copy_dir="$tmp_root/remote-rewritten-head-skill"
  local request_review_script="$skill_copy_dir/scripts/request-review"
  local output
  local review_sha
  local expected
  local pr_exists_file="$tmp_root/remote-rewritten-head-pr-open"
  local remote_sha

  setup_repo "$repo_dir" "$origin_dir"
  setup_common_home "$home_dir"
  setup_common_stubs "$bin_dir"
  setup_remote_review_stubs "$bin_dir"
  make_skill_copy "$skill_copy_dir" 'REQUEST_REVIEW_MODE=remote'

  printf 'remote history commit\n' >>"$repo_dir/file.txt"
  git -C "$repo_dir" add file.txt
  git -C "$repo_dir" commit -m "remote history commit" >/dev/null
  git -C "$repo_dir" push origin feature/request-review-artifact >/dev/null

  git -C "$repo_dir" reset --hard HEAD~1 >/dev/null
  printf 'rewritten local head\n' >>"$repo_dir/file.txt"
  git -C "$repo_dir" add file.txt
  git -C "$repo_dir" commit -m "rewritten local head" >/dev/null
  review_sha="$(git -C "$repo_dir" rev-parse HEAD)"

  output="$(
    cd "$repo_dir" &&
      HOME="$home_dir" \
      PATH="$bin_dir:$PATH" \
      REQUEST_REVIEW_TEST_PR_EXISTS_FILE="$pr_exists_file" \
      REQUEST_REVIEW_EXPECTED_TRIGGER_TIME=2026-03-08T00:00:00Z \
      "$request_review_script" "test: remote rewritten head"
  )"

  expected="request-review (remote): 👍 from chatgpt-codex-connector[bot] on PR #711 after commit $review_sha"
  [[ "$output" == *"$expected"* ]] || fail "unexpected rewritten-head output: $output"
  remote_sha="$(git --git-dir="$origin_dir" rev-parse refs/heads/feature/request-review-artifact)"
  [[ "$remote_sha" == "$review_sha" ]] || fail "expected remote branch to force-update to $review_sha, got $remote_sha"
  assert_file_contains "$repo_dir/review.log" "$expected"
}

test_remote_existing_commit_from_stale_clone_does_not_rewind_remote() {
  local repo_dir="$tmp_root/remote-stale-clone-repo"
  local origin_dir="$tmp_root/remote-stale-clone-origin.git"
  local home_dir="$tmp_root/remote-stale-clone-home"
  local bin_dir="$tmp_root/remote-stale-clone-bin"
  local skill_copy_dir="$tmp_root/remote-stale-clone-skill"
  local request_review_script="$skill_copy_dir/scripts/request-review"
  local output
  local review_sha
  local remote_sha
  local other_clone_dir="$tmp_root/remote-stale-clone-other"
  local pr_exists_file="$tmp_root/remote-stale-clone-pr-open"
  local expected

  setup_repo "$repo_dir" "$origin_dir"
  setup_common_home "$home_dir"
  setup_common_stubs "$bin_dir"
  setup_remote_review_stubs "$bin_dir"
  make_skill_copy "$skill_copy_dir" 'REQUEST_REVIEW_MODE=remote'

  printf 'stale clone review target\n' >>"$repo_dir/file.txt"
  git -C "$repo_dir" add file.txt
  git -C "$repo_dir" commit -m "stale clone review target" >/dev/null
  git -C "$repo_dir" push origin feature/request-review-artifact >/dev/null
  review_sha="$(git -C "$repo_dir" rev-parse HEAD)"

  git clone "$origin_dir" "$other_clone_dir" >/dev/null
  git -C "$other_clone_dir" checkout feature/request-review-artifact >/dev/null
  git -C "$other_clone_dir" config user.name "Codex Test"
  git -C "$other_clone_dir" config user.email "codex@example.com"
  printf 'newer remote-only commit\n' >>"$other_clone_dir/file.txt"
  git -C "$other_clone_dir" add file.txt
  git -C "$other_clone_dir" commit -m "newer remote-only commit" >/dev/null
  git -C "$other_clone_dir" push origin feature/request-review-artifact >/dev/null
  remote_sha="$(git --git-dir="$origin_dir" rev-parse refs/heads/feature/request-review-artifact)"

  output="$(
    cd "$repo_dir" &&
      HOME="$home_dir" \
      PATH="$bin_dir:$PATH" \
      REQUEST_REVIEW_TEST_PR_EXISTS_FILE="$pr_exists_file" \
      REQUEST_REVIEW_EXPECTED_TRIGGER_TIME=2026-03-08T00:00:00Z \
      "$request_review_script" --use-existing-commit --existing-commit "$review_sha" "test: stale clone existing commit"
  )"

  expected="request-review (remote): 👍 from chatgpt-codex-connector[bot] on PR #711 after commit $review_sha"
  [[ "$output" == *"$expected"* ]] || fail "unexpected stale-clone existing-commit output: $output"
  [[ "$(git --git-dir="$origin_dir" rev-parse refs/heads/feature/request-review-artifact)" == "$remote_sha" ]] || fail "expected remote branch to remain at $remote_sha"
  assert_file_contains "$repo_dir/review.log" "$expected"
}

test_stale_lock_is_reclaimed_before_review_runs() {
  local repo_dir="$tmp_root/stale-lock-repo"
  local origin_dir="$tmp_root/stale-lock-origin.git"
  local home_dir="$tmp_root/stale-lock-home"
  local bin_dir="$tmp_root/stale-lock-bin"
  local skill_copy_dir="$tmp_root/stale-lock-skill"
  local request_review_script="$skill_copy_dir/scripts/request-review"
  local output
  local stale_lock_dir

  setup_repo "$repo_dir" "$origin_dir"
  setup_common_home "$home_dir"
  setup_common_stubs "$bin_dir"
  setup_local_success_review_stubs "$home_dir" "$bin_dir"
  make_skill_copy "$skill_copy_dir" 'REQUEST_REVIEW_MODE=local'

  stale_lock_dir="$(
    python3 - <<'PY' "$home_dir"
import hashlib
import sys
home_dir = sys.argv[1]
scope = "mode=local|repo=stale-lock-repo|branch=feature/request-review-artifact"
print(f"{home_dir}/.codex/tmp/request-review.lock.{hashlib.sha256(scope.encode('utf-8')).hexdigest()[:20]}")
PY
  )"

  mkdir -p "$stale_lock_dir"
  cat >"$stale_lock_dir/owner" <<'EOF'
pid=999999
started=2026-03-11T00:00:00Z
scope=mode=local|repo=stale-lock-repo|branch=feature/request-review-artifact
EOF

  output="$(
    cd "$repo_dir" &&
      HOME="$home_dir" \
      PATH="$bin_dir:$PATH" \
      "$request_review_script" --use-existing-commit "test: stale lock reclaimed"
  )"

  [[ "$output" == *"all clear!"* ]] || fail "unexpected stale lock output: $output"
  assert_file_contains "$repo_dir/review.log" "all clear!"
  [[ ! -d "$stale_lock_dir" ]] || fail "expected stale lock dir to be removed"
}

test_local_review_mounts_worktree_git_metadata_into_docker() {
  local repo_dir="$tmp_root/worktree-mount-repo"
  local origin_dir="$tmp_root/worktree-mount-origin.git"
  local worktree_dir="$tmp_root/worktree-mount-checkout"
  local home_dir="$tmp_root/worktree-mount-home"
  local bin_dir="$tmp_root/worktree-mount-bin"
  local skill_copy_dir="$tmp_root/worktree-mount-skill"
  local request_review_script="$skill_copy_dir/scripts/request-review"
  local capture_path="$tmp_root/worktree-mount-docker-args.txt"
  local absolute_git_dir
  local common_git_dir
  local output

  setup_repo "$repo_dir" "$origin_dir"
  setup_common_home "$home_dir"
  setup_common_stubs "$bin_dir"
  setup_local_capture_review_stubs "$home_dir" "$bin_dir" "$capture_path"
  make_skill_copy "$skill_copy_dir" 'REQUEST_REVIEW_MODE=local'

  git -C "$repo_dir" branch worktree-review HEAD >/dev/null
  git -C "$repo_dir" worktree add "$worktree_dir" worktree-review >/dev/null

  absolute_git_dir="$(git -C "$worktree_dir" rev-parse --absolute-git-dir)"
  common_git_dir="$(git -C "$worktree_dir" rev-parse --git-common-dir)"

  output="$(
    cd "$worktree_dir" &&
      HOME="$home_dir" \
      PATH="$bin_dir:$PATH" \
      "$request_review_script" --use-existing-commit "test: worktree git mounts"
  )"

  [[ "$output" == *"all clear!"* ]] || fail "unexpected local worktree output: $output"
  assert_file_includes "$capture_path" "$absolute_git_dir:$absolute_git_dir:ro"
  assert_file_includes "$capture_path" "$common_git_dir:$common_git_dir:ro"
  assert_file_includes "$capture_path" "/codex-home/skills:rw"
}

test_remote_disable_writes_review_log
test_local_failure_clears_review_log
test_remote_rerun_reuses_head_when_pr_is_open
test_remote_rerun_pushes_head_when_pr_branch_is_behind
test_remote_terminal_issue_comment_exits_nonzero
test_use_existing_commit_flag_reviews_clean_head
test_existing_commit_flag_like_text_stays_in_message
test_canonical_env_overrides_agent_runtime_env
test_remote_dirty_worktree_creates_pr_and_reviews_in_one_shot
test_remote_clean_head_without_pr_reuses_head_and_creates_pr
test_remote_existing_commit_pushes_selected_sha_not_head
test_remote_existing_ancestor_commit_skips_non_fast_forward_push
test_remote_rewritten_head_force_pushes_with_lease
test_remote_existing_commit_from_stale_clone_does_not_rewind_remote
test_stale_lock_is_reclaimed_before_review_runs
test_local_review_mounts_worktree_git_metadata_into_docker

echo "PASS: request-review artifact handling"

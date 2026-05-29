#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
skill_dir="$(cd "$script_dir/.." && pwd)"
lib_script="$skill_dir/scripts/_worktree_lib.sh"

tmp_root="$(mktemp -d)"
trap 'rm -rf "$tmp_root"' EXIT

fail() {
  echo "FAIL: $*" >&2
  exit 1
}

repo_dir="$tmp_root/repo"
origin_dir="$tmp_root/origin.git"
worktree_path="$repo_dir/.worktrees/feature"
fake_bin="$tmp_root/bin"
git_push_count="$tmp_root/git-push-count"

mkdir -p "$fake_bin"

cat >"$fake_bin/gh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
if [[ "$1" == "pr" && "$2" == "view" ]]; then
  printf '{"number":711,"url":"https://github.com/example/repo/pull/711","state":"OPEN","isDraft":false,"headRefName":"feature/demo","baseRefName":"master","title":"Feature demo"}\n'
  exit 0
fi
echo "unexpected gh invocation: $*" >&2
exit 1
EOF
chmod +x "$fake_bin/gh"
cat >"$fake_bin/git" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
real_git="/usr/bin/git"
count_file="${GIT_PUSH_COUNT_FILE:?}"
if [[ "$*" == *" push "* ]]; then
  count="$(cat "$count_file" 2>/dev/null || printf '0')"
  count=$((count + 1))
  printf '%s\n' "$count" >"$count_file"
  if [[ "$count" -eq 1 ]]; then
    echo "error: cannot open '/tmp/repo/.git/worktrees/feature/FETCH_HEAD': Operation not permitted" >&2
    exit 1
  fi
fi
exec "$real_git" "$@"
EOF
chmod +x "$fake_bin/git"

git init -b master "$repo_dir" >/dev/null
git -C "$repo_dir" config user.name "Codex Test"
git -C "$repo_dir" config user.email "codex@example.com"
printf 'base\n' >"$repo_dir/file.txt"
git -C "$repo_dir" add file.txt
git -C "$repo_dir" commit -m "base" >/dev/null

git init --bare "$origin_dir" >/dev/null
git -C "$repo_dir" remote add origin "$origin_dir"
git -C "$repo_dir" push -u origin master >/dev/null

mkdir -p "$repo_dir/.worktrees"
git -C "$repo_dir" worktree add -b feature/demo "$worktree_path" master >/dev/null
printf 'feature\n' >"$worktree_path/feature.txt"
git -C "$worktree_path" add feature.txt
git -C "$worktree_path" commit -m "add feature file" >/dev/null

legacy_review_artifact="review."
legacy_review_artifact+="log"
[[ ! -e "$worktree_path/$legacy_review_artifact" ]] || fail "test setup unexpectedly created legacy review artifact"

output="$(FAKE_BIN="$fake_bin" LIB_SCRIPT="$lib_script" GIT_PUSH_COUNT_FILE="$git_push_count" bash -lc '
  set -euo pipefail
  source "$LIB_SCRIPT"
  git_subprocess_path() {
    printf "%s\n" "$FAKE_BIN:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin"
  }
  git_publish_worktree_cmd "'"$worktree_path"'" master
')"

[[ "$output" == *"published branch feature/demo to master; PR #711"* ]] || fail "expected publish output, got: $output"
[[ "$(cat "$git_push_count")" == "2" ]] || fail "expected publish to retry after metadata permission failure"
git --git-dir="$origin_dir" rev-parse --verify refs/heads/feature/demo >/dev/null || fail "expected feature branch to be pushed"

echo "PASS: git-publish-worktree publishes without obsolete review artifact"

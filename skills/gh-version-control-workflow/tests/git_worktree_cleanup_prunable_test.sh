#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
skill_dir="$(cd "$script_dir/.." && pwd)"
cleanup_script="$skill_dir/scripts/git-worktree-cleanup"
create_script="$skill_dir/scripts/git-worktree-create"

tmp_root="$(mktemp -d)"
trap 'rm -rf "$tmp_root"' EXIT

fail() {
  echo "FAIL: $*" >&2
  exit 1
}

repo_dir="$tmp_root/repo"
origin_dir="$tmp_root/origin.git"
worktree_name="legacy-worker"
worktree_path="$repo_dir/.worktrees/$worktree_name"
branch_name="legacy/worker"

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
git -C "$repo_dir" worktree add -b "$branch_name" "$worktree_path" master >/dev/null
worktree_path="$(python3 -c 'from pathlib import Path; import sys; print(Path(sys.argv[1]).resolve())' "$worktree_path")"
rm -rf "$worktree_path"

worktree_list="$(git -C "$repo_dir" worktree list --porcelain)"
[[ "$worktree_list" == *"$worktree_path"* ]] || fail "expected stale worktree registration before cleanup"

output="$("$cleanup_script" "$worktree_path" master)"
[[ "$output" == *"removed stale worktree registration"* ]] || fail "expected stale registration cleanup output, got: $output"
worktree_list="$(git -C "$repo_dir" worktree list --porcelain)"
if [[ "$worktree_list" == *"$worktree_path"* ]]; then
  fail "expected stale worktree registration to be removed"
fi
if git -C "$repo_dir" show-ref --verify --quiet "refs/heads/$branch_name"; then
  fail "expected stale local branch to be deleted"
fi

create_output="$("$create_script" "$repo_dir" master "$branch_name" "$worktree_name")"
[[ "$create_output" == *"created worktree"* ]] || fail "expected recreate output, got: $create_output"
[[ -d "$worktree_path" ]] || fail "expected worktree path to be recreated"
[[ "$(git -C "$worktree_path" rev-parse --abbrev-ref HEAD)" == "$branch_name" ]] || fail "expected recreated worktree branch"

echo "PASS: git-worktree-cleanup removes stale missing worktree metadata and allows recreate"

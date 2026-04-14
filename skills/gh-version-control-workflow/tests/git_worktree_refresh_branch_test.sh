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

repo_dir="$tmp_root/repo"
origin_dir="$tmp_root/origin.git"
other_clone_dir="$tmp_root/other"
worktree_path="$repo_dir/.worktrees/designer"
refresh_script="$skill_dir/scripts/git-worktree-refresh-branch"

git init -b main "$repo_dir" >/dev/null
git -C "$repo_dir" config user.name "Codex Test"
git -C "$repo_dir" config user.email "codex@example.com"
printf 'base\n' >"$repo_dir/file.txt"
git -C "$repo_dir" add file.txt
git -C "$repo_dir" commit -m "base" >/dev/null

git init --bare "$origin_dir" >/dev/null
git -C "$repo_dir" remote add origin "$origin_dir"
git -C "$repo_dir" push -u origin main >/dev/null

mkdir -p "$repo_dir/.worktrees"
git -C "$repo_dir" worktree add -b design/old "$worktree_path" main >/dev/null

git clone "$origin_dir" "$other_clone_dir" >/dev/null
git -C "$other_clone_dir" checkout main >/dev/null
git -C "$other_clone_dir" config user.name "Codex Test"
git -C "$other_clone_dir" config user.email "codex@example.com"
printf 'remote advance\n' >>"$other_clone_dir/file.txt"
git -C "$other_clone_dir" add file.txt
git -C "$other_clone_dir" commit -m "advance main" >/dev/null
git -C "$other_clone_dir" push origin main >/dev/null

output="$("$refresh_script" "$worktree_path" "design/fresh")"

[[ "$output" == *"origin/main"* ]] || fail "expected refresh output to mention origin/main, got: $output"
[[ "$(git -C "$worktree_path" rev-parse --abbrev-ref HEAD)" == "design/fresh" ]] || fail "expected worktree to switch to design/fresh"
[[ "$(git -C "$worktree_path" log -1 --pretty=%s)" == "advance main" ]] || fail "expected fresh branch to start at latest origin/main"

if "$refresh_script" "$worktree_path" "main" >/dev/null 2>&1; then
  fail "expected protected branch target to be rejected"
fi

echo "PASS: git-worktree-refresh-branch creates a fresh non-integration branch from latest origin/main"

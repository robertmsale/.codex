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
worktrees_dir="$repo_dir/.worktrees"
worktree_path="$worktrees_dir/feature-cleanup"
cleanup_script="$skill_dir/scripts/git-worktree-cleanup"

git init -b main "$repo_dir" >/dev/null
git -C "$repo_dir" config user.name "Codex Test"
git -C "$repo_dir" config user.email "codex@example.com"
printf 'base\n' >"$repo_dir/file.txt"
printf 'notes\n' >"$repo_dir/notes.txt"
git -C "$repo_dir" add file.txt notes.txt
git -C "$repo_dir" commit -m "base" >/dev/null

git init --bare "$origin_dir" >/dev/null
git -C "$repo_dir" remote add origin "$origin_dir"
git -C "$repo_dir" push -u origin main >/dev/null

mkdir -p "$worktrees_dir"
git -C "$repo_dir" worktree add -b feature-cleanup "$worktree_path" main >/dev/null

printf 'dirty integration change\n' >>"$repo_dir/notes.txt"
printf 'keep me\n' >"$repo_dir/local-note.txt"

git clone "$origin_dir" "$other_clone_dir" >/dev/null
git -C "$other_clone_dir" checkout main >/dev/null
git -C "$other_clone_dir" config user.name "Codex Test"
git -C "$other_clone_dir" config user.email "codex@example.com"
printf 'remote main advance\n' >>"$other_clone_dir/file.txt"
git -C "$other_clone_dir" add file.txt
git -C "$other_clone_dir" commit -m "remote main advance" >/dev/null
git -C "$other_clone_dir" push origin main >/dev/null

output="$("$cleanup_script" "$worktree_path")"

[[ "$output" == *"origin/main"* ]] || fail "expected cleanup output to mention origin/main, got: $output"
[[ ! -e "$worktree_path" ]] || fail "expected worktree to be removed"
if git -C "$repo_dir" worktree list --porcelain | awk '/^worktree / {sub(/^worktree /, ""); print}' | grep -Fxq "$worktree_path"; then
  fail "expected worktree metadata to be pruned for $worktree_path"
fi
[[ "$(git -C "$repo_dir" rev-parse --abbrev-ref HEAD)" == "main" ]] || fail "expected repo root to stay on main"
[[ "$(git -C "$repo_dir" log -1 --pretty=%s)" == "remote main advance" ]] || fail "expected repo root to fast-forward to remote main"
[[ "$(tail -n 1 "$repo_dir/notes.txt")" == "dirty integration change" ]] || fail "expected dirty tracked change to be restored after stash pop"
[[ "$(cat "$repo_dir/local-note.txt")" == "keep me" ]] || fail "expected untracked file to be restored after stash pop"

echo "PASS: git-worktree-cleanup syncs integration branch and removes worktree metadata"

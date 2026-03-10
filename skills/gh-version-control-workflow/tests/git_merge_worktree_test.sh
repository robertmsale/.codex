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
merge_clone_dir="$tmp_root/merge-clone"
worktrees_dir="$repo_dir/.worktrees"
worktree_path="$worktrees_dir/feature-merge"
merge_script="$skill_dir/scripts/git-merge-worktree"
fake_bin="$tmp_root/bin"
fake_gh="$fake_bin/gh"

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
git -C "$repo_dir" worktree add -b feature-merge "$worktree_path" main >/dev/null
printf 'feature branch change\n' >>"$worktree_path/file.txt"
git -C "$worktree_path" add file.txt
git -C "$worktree_path" commit -m "feature change" >/dev/null
git -C "$worktree_path" push -u origin feature-merge >/dev/null

printf 'dirty integration change\n' >>"$repo_dir/notes.txt"
printf 'keep me\n' >"$repo_dir/local-note.txt"

mkdir -p "$fake_bin"
cat >"$fake_gh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

if [[ "${1:-}" == "pr" && "${2:-}" == "view" ]]; then
  printf '14\n'
  exit 0
fi

if [[ "${1:-}" == "pr" && "${2:-}" == "merge" ]]; then
  branch="${FAKE_GH_BRANCH:?}"
  base="${FAKE_GH_BASE:?}"
  origin_dir="${FAKE_GH_ORIGIN:?}"
  merge_clone_dir="${FAKE_GH_MERGE_CLONE:?}"

  rm -rf "$merge_clone_dir"
  git clone "$origin_dir" "$merge_clone_dir" >/dev/null 2>&1
  git -C "$merge_clone_dir" config user.name "Codex Test"
  git -C "$merge_clone_dir" config user.email "codex@example.com"
  git -C "$merge_clone_dir" checkout "$base" >/dev/null 2>&1
  git -C "$merge_clone_dir" fetch origin "$branch" >/dev/null 2>&1
  git -C "$merge_clone_dir" merge --squash "origin/$branch" >/dev/null 2>&1
  git -C "$merge_clone_dir" commit -m "squash merge $branch" >/dev/null 2>&1
  git -C "$merge_clone_dir" push origin "$base" >/dev/null 2>&1
  exit 0
fi

echo "unexpected gh invocation: $*" >&2
exit 1
EOF
chmod +x "$fake_gh"

output="$(PATH="$fake_bin:$PATH" FAKE_GH_BRANCH="feature-merge" FAKE_GH_BASE="main" FAKE_GH_ORIGIN="$origin_dir" FAKE_GH_MERGE_CLONE="$merge_clone_dir" "$merge_script" "$worktree_path")"

[[ "$output" == *"Squash-merged PR #14"* ]] || fail "expected merge output to mention PR #14, got: $output"
[[ "$output" == *"origin/feature-merge"* ]] || fail "expected merge output to mention deleted remote branch, got: $output"
[[ ! -e "$worktree_path" ]] || fail "expected worktree to be removed"
if git -C "$repo_dir" worktree list --porcelain | awk '/^worktree / {sub(/^worktree /, ""); print}' | grep -Fxq "$worktree_path"; then
  fail "expected worktree metadata to be pruned for $worktree_path"
fi
if git -C "$repo_dir" show-ref --verify --quiet "refs/heads/feature-merge"; then
  fail "expected local feature branch to be deleted"
fi
if git --git-dir="$origin_dir" show-ref --verify --quiet "refs/heads/feature-merge"; then
  fail "expected remote feature branch to be deleted"
fi
if git -C "$repo_dir" show-ref --verify --quiet "refs/remotes/origin/feature-merge"; then
  fail "expected remote tracking ref for feature branch to be pruned"
fi
[[ "$(git -C "$repo_dir" rev-parse --abbrev-ref HEAD)" == "main" ]] || fail "expected repo root to stay on main"
[[ "$(git -C "$repo_dir" log -1 --pretty=%s)" == "squash merge feature-merge" ]] || fail "expected repo root to fast-forward to squash merge commit"
[[ "$(tail -n 1 "$repo_dir/notes.txt")" == "dirty integration change" ]] || fail "expected dirty tracked change to be restored after stash pop"
[[ "$(cat "$repo_dir/local-note.txt")" == "keep me" ]] || fail "expected untracked file to be restored after stash pop"

echo "PASS: git-merge-worktree squash merges, deletes branch refs, and cleans the worktree"

tmp_root_fail="$(mktemp -d)"
trap 'rm -rf "$tmp_root" "$tmp_root_fail"' EXIT

repo_dir_fail="$tmp_root_fail/repo"
origin_dir_fail="$tmp_root_fail/origin.git"
worktrees_dir_fail="$repo_dir_fail/.worktrees"
worktree_path_fail="$worktrees_dir_fail/feature-merge-fail"
fake_bin_fail="$tmp_root_fail/bin"
fake_gh_fail="$fake_bin_fail/gh"

git init -b main "$repo_dir_fail" >/dev/null
git -C "$repo_dir_fail" config user.name "Codex Test"
git -C "$repo_dir_fail" config user.email "codex@example.com"
printf 'base\n' >"$repo_dir_fail/file.txt"
git -C "$repo_dir_fail" add file.txt
git -C "$repo_dir_fail" commit -m "base" >/dev/null

git init --bare "$origin_dir_fail" >/dev/null
git -C "$repo_dir_fail" remote add origin "$origin_dir_fail"
git -C "$repo_dir_fail" push -u origin main >/dev/null

mkdir -p "$worktrees_dir_fail"
git -C "$repo_dir_fail" worktree add -b feature-merge-fail "$worktree_path_fail" main >/dev/null
printf 'feature branch change\n' >>"$worktree_path_fail/file.txt"
git -C "$worktree_path_fail" add file.txt
git -C "$worktree_path_fail" commit -m "feature change" >/dev/null
git -C "$worktree_path_fail" push -u origin feature-merge-fail >/dev/null

mkdir -p "$fake_bin_fail"
cat >"$fake_gh_fail" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

if [[ "${1:-}" == "pr" && "${2:-}" == "view" ]]; then
  printf '15\n'
  exit 0
fi

if [[ "${1:-}" == "pr" && "${2:-}" == "merge" ]]; then
  echo "merge conflict" >&2
  exit 1
fi

echo "unexpected gh invocation: $*" >&2
exit 1
EOF
chmod +x "$fake_gh_fail"

if PATH="$fake_bin_fail:$PATH" "$merge_script" "$worktree_path_fail" >/dev/null 2>&1; then
  fail "expected merge script to fail when gh merge fails"
fi

[[ -e "$worktree_path_fail" ]] || fail "expected failed merge to leave worktree in place"
[[ "$(git -C "$worktree_path_fail" rev-parse --abbrev-ref HEAD)" == "feature-merge-fail" ]] || fail "expected failed merge to leave linked worktree usable"
if ! git -C "$repo_dir_fail" show-ref --verify --quiet "refs/heads/feature-merge-fail"; then
  fail "expected failed merge to leave local feature branch in place"
fi
if ! git --git-dir="$origin_dir_fail" show-ref --verify --quiet "refs/heads/feature-merge-fail"; then
  fail "expected failed merge to leave remote feature branch in place"
fi

echo "PASS: git-merge-worktree leaves branch and worktree intact when squash merge fails"

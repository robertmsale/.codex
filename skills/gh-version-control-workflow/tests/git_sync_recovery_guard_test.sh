#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
skill_dir="$(cd "$script_dir/.." && pwd)"
lib_script="$skill_dir/scripts/_worktree_lib.sh"
sync_script="$skill_dir/scripts/git-sync-worktree"

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

mkdir -p "$fake_bin"
cat >"$fake_bin/gh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
if [[ "$1" == "pr" && "$2" == "view" ]]; then
  exit 1
fi
echo "unexpected gh invocation: $*" >&2
exit 1
EOF
chmod +x "$fake_bin/gh"

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
printf 'dirty\n' >"$worktree_path/dirty.txt"

sync_error="$tmp_root/sync.err"
if "$sync_script" "$worktree_path" master >"$tmp_root/sync.out" 2>"$sync_error"; then
  fail "expected dirty git-sync-worktree to be refused"
fi
[[ "$(cat "$sync_error")" == *"Refusing to sync a dirty worktree"* ]] || fail "expected dirty sync refusal, got: $(cat "$sync_error")"
[[ "$(git -C "$repo_dir" stash list)" != *"stash@"* ]] || fail "expected dirty sync refusal not to create a stash"
[[ -e "$worktree_path/dirty.txt" ]] || fail "expected dirty file to remain in place"

git -C "$worktree_path" add dirty.txt
git -C "$worktree_path" commit -m "add dirty file as feature work" >/dev/null

recover_error="$tmp_root/recover.err"
if FAKE_BIN="$fake_bin" LIB_SCRIPT="$lib_script" WORKTREE_PATH="$worktree_path" bash -lc '
  set -euo pipefail
  source "$LIB_SCRIPT"
  git_subprocess_path() {
    printf "%s\n" "$FAKE_BIN:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin"
  }
  git_recover_published_worktree_cmd "$WORKTREE_PATH" master
' >"$tmp_root/recover.out" 2>"$recover_error"; then
  fail "expected unpublished recovery to be refused"
fi
[[ "$(cat "$recover_error")" == *"Refusing published PR recovery because no PR was found"* ]] || fail "expected unpublished recovery refusal, got: $(cat "$recover_error")"

echo "PASS: git sync/recovery guards refuse dirty or unpublished stale worktrees"

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

mkdir -p "$fake_bin"

cat >"$fake_bin/gh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
if [[ "$1" == "pr" && "$2" == "view" ]]; then
  if [[ "$3" == "feature/demo" && "$4" == "--json" && "$5" == "number" && "$6" == "--jq" && "$7" == ".number" ]]; then
    printf '42\n'
    exit 0
  fi
fi
if [[ "$1" == "pr" && "$2" == "merge" && "$3" == "42" && "$4" == "--squash" ]]; then
  exit 0
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
printf 'reviewed\n' >"$worktree_path/review.log"
git -C "$worktree_path" add review.log
git -C "$worktree_path" commit -m "add review log" >/dev/null
git -C "$worktree_path" push -u origin feature/demo >/dev/null

printf 'remote advance\n' >>"$repo_dir/file.txt"
git -C "$repo_dir" add file.txt
git -C "$repo_dir" commit -m "advance master" >/dev/null
git -C "$repo_dir" push origin master >/dev/null
git -C "$repo_dir" reset --hard HEAD~1 >/dev/null

output="$(FAKE_BIN="$fake_bin" LIB_SCRIPT="$lib_script" bash -lc '
  set -euo pipefail
  source "$LIB_SCRIPT"
  git_subprocess_path() {
    printf "%s\n" "$FAKE_BIN:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin"
  }
  git_merge_worktree_cmd "'"$worktree_path"'" master
')"

[[ "$output" == *"fast-forwarded local master to origin/master"* ]] || fail "expected merge output to mention fast-forwarded local master, got: $output"
[[ "$(git -C "$repo_dir" log -1 --pretty=%s)" == "advance master" ]] || fail "expected local master to fast-forward to origin/master"
[[ ! -e "$worktree_path" ]] || fail "expected worktree path to be removed"

echo "PASS: git-merge-worktree fast-forwards local integration branch and removes the worktree path"

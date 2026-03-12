#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
skill_dir="$(cd "$script_dir/.." && pwd)"
parser_script="$skill_dir/scripts/command-parser"

tmp_root="$(mktemp -d)"
trap 'rm -rf "$tmp_root"' EXIT

fail() {
  echo "FAIL: $*" >&2
  exit 1
}

assert_file_includes() {
  local path="$1"
  local expected_fragment="$2"

  [[ -f "$path" ]] || fail "expected file to exist: $path"
  local actual
  actual="$(cat "$path")"
  [[ "$actual" == *"$expected_fragment"* ]] || fail "expected '$expected_fragment' in $path, got '$actual'"
}

assert_file_excludes() {
  local path="$1"
  local forbidden_fragment="$2"

  [[ -f "$path" ]] || fail "expected file to exist: $path"
  local actual
  actual="$(cat "$path")"
  [[ "$actual" != *"$forbidden_fragment"* ]] || fail "did not expect '$forbidden_fragment' in $path, got '$actual'"
}

repo_dir="$tmp_root/repo"
bin_dir="$tmp_root/bin"
home_dir="$tmp_root/home"
capture_path="$tmp_root/docker-argv.txt"

mkdir -p "$repo_dir" "$bin_dir" "$home_dir/.codex"
printf 'base\n' >"$repo_dir/file.txt"

cat >"$bin_dir/codex" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
if [[ "${1:-}" == "execpolicy" && "${2:-}" == "check" ]]; then
  echo '{"decision":"allow","matchedRules":[]}'
  exit 0
fi
exit 0
EOF
chmod +x "$bin_dir/codex"

cat >"$bin_dir/docker" <<EOF
#!/usr/bin/env bash
set -euo pipefail

if [[ "\${1:-}" == "image" && "\${2:-}" == "inspect" ]]; then
  exit 0
fi

if [[ "\${1:-}" == "run" ]]; then
  printf '%s\n' "\$@" >"$capture_path"
  printf '%s\n' '{"type":"item.completed","item":{"type":"agent_message","text":"No errors!"}}'
  exit 0
fi

exit 2
EOF
chmod +x "$bin_dir/docker"

output="$(
  cd "$repo_dir" &&
    HOME="$home_dir" \
    PATH="$bin_dir:$PATH" \
    "$parser_script" /usr/bin/printf 'ok\n'
)"

[[ "$output" == "No errors!" ]] || fail "unexpected parser output: $output"
assert_file_includes "$capture_path" "/codex-home/skills:ro"
assert_file_excludes "$capture_path" "$repo_dir:/workspace:rw"

echo "PASS: command-parser container scope"

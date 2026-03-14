#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
skill_dir="$(cd "$script_dir/.." && pwd)"
parser_script="$skill_dir/scripts/command-parser"
expected_codex_home="$(cd "$skill_dir/../.." && pwd)"

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
log_path="$tmp_root/docker.log"
state_dir="$tmp_root/docker-state"
spool_root="/tmp/command-parser-spool"

mkdir -p "$repo_dir" "$bin_dir" "$home_dir/.codex" "$state_dir"
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

log_path="$log_path"
state_dir="$state_dir"
container_name_file="\$state_dir/container-name"
container_image_file="\$state_dir/container-image"
container_running_file="\$state_dir/container-running"
spool_host_file="\$state_dir/spool-host"

if [[ "\${1:-}" == "image" && "\${2:-}" == "inspect" ]]; then
  if [[ "\${3:-}" == "-f" ]]; then
    case "\${5:-}" in
      command-parser:0.110.0)
        printf 'sha256:parser-default\n'
        ;;
      command-parser:9.9.9)
        printf 'sha256:parser-alt\n'
        ;;
      *)
        printf 'sha256:parser-unknown\n'
        ;;
    esac
  fi
  exit 0
fi

if [[ "\${1:-}" == "container" && "\${2:-}" == "inspect" ]]; then
  shift 2
  format=""
  if [[ "\${1:-}" == "-f" ]]; then
    format="\$2"
    shift 2
  fi
  name="\${1:-}"
  [[ -f "\$container_name_file" ]] || exit 1
  [[ "\$name" == "\$(cat "\$container_name_file")" ]] || exit 1
  case "\$format" in
    '{{.Config.Image}}')
      cat "\$container_image_file"
      ;;
    '{{.Image}}')
      image_tag="\$(cat "\$container_image_file")"
      case "\$image_tag" in
        command-parser:0.110.0)
          printf 'sha256:parser-default\n'
          ;;
        command-parser:9.9.9)
          printf 'sha256:parser-alt\n'
          ;;
        *)
          printf 'sha256:parser-unknown\n'
          ;;
      esac
      ;;
    '{{.State.Running}}')
      cat "\$container_running_file"
      ;;
    *)
      printf '{}\n'
      ;;
  esac
  exit 0
fi

if [[ "\${1:-}" == "rm" && "\${2:-}" == "-f" ]]; then
  printf 'rm %s\n' "\$*" >>"\$log_path"
  rm -f "\$container_name_file" "\$container_image_file" "\$container_running_file"
  exit 0
fi

if [[ "\${1:-}" == "start" ]]; then
  printf 'start %s\n' "\$*" >>"\$log_path"
  printf 'true' >"\$container_running_file"
  exit 0
fi

if [[ "\${1:-}" == "run" ]]; then
  printf 'run %s\n' "\$*" >>"\$log_path"
  shift
  image=""
  name=""
  while [[ \$# -gt 0 ]]; do
    case "\$1" in
      --name)
        name="\$2"
        shift 2
        ;;
      -v)
        mount_spec="\$2"
        if [[ "\$mount_spec" == *":/tmp/command-parser-spool:rw" ]]; then
          printf '%s' "\${mount_spec%%:/tmp/command-parser-spool:rw}" >"\$spool_host_file"
        fi
        shift 2
        ;;
      -e|-u|--workdir)
        shift 2
        ;;
      -d)
        shift
        ;;
      *)
        image="\$1"
        break
        ;;
    esac
  done
  printf '%s' "\$name" >"\$container_name_file"
  printf '%s' "\$image" >"\$container_image_file"
  printf 'true' >"\$container_running_file"
  exit 0
fi

if [[ "\${1:-}" == "exec" ]]; then
  printf 'exec %s\n' "\$*" >>"\$log_path"
  shift
  workdir=""
  while [[ \$# -gt 0 ]]; do
    case "\$1" in
      -e|-u)
        shift 2
        ;;
      --workdir)
        workdir="\$2"
        shift 2
        ;;
      *)
        container_name="\$1"
        shift
        break
        ;;
    esac
  done
  [[ "\$container_name" == "\$(cat "\$container_name_file")" ]] || exit 1
  spool_host="\$(cat "\$spool_host_file")"
  host_workdir="\${workdir/\/tmp\/command-parser-spool/\$spool_host}"
  mkdir -p "\$host_workdir"
  printf '%s\n' '{"type":"item.completed","item":{"type":"agent_message","text":"No errors!"}}' >"\$host_workdir/events.jsonl"
  exit 0
fi

exit 2
EOF
chmod +x "$bin_dir/docker"

output_one="$(
  cd "$repo_dir" &&
    HOME="$home_dir" \
    PATH="$bin_dir:$PATH" \
    "$parser_script" /usr/bin/printf 'ok\n'
)"

output_two="$(
  cd "$repo_dir" &&
    HOME="$home_dir" \
    PATH="$bin_dir:$PATH" \
    "$parser_script" /usr/bin/printf 'ok\n'
)"

fresh_codex_home="$tmp_root/fresh-codex-home"
output_three="$(
  cd "$repo_dir" &&
    HOME="$home_dir" \
    CODEX_HOME="$fresh_codex_home" \
    PATH="$bin_dir:$PATH" \
    "$parser_script" /usr/bin/printf 'ok\n'
)"

image_warning_path="$tmp_root/image-warning.txt"
output_four="$(
  cd "$repo_dir" &&
    HOME="$home_dir" \
    COMMAND_PARSER_IMAGE="command-parser:9.9.9" \
    PATH="$bin_dir:$PATH" \
    "$parser_script" /usr/bin/printf 'ok\n' 2>"$image_warning_path"
)"

[[ "$output_one" == "No errors!" ]] || fail "unexpected parser output: $output_one"
[[ "$output_two" == "No errors!" ]] || fail "unexpected parser output: $output_two"
[[ "$output_three" == "No errors!" ]] || fail "unexpected parser output: $output_three"
[[ "$output_four" == "No errors!" ]] || fail "unexpected parser output: $output_four"
assert_file_includes "$log_path" "/codex-home/skills:rw"
assert_file_includes "$log_path" "$spool_root:/tmp/command-parser-spool:rw"
assert_file_includes "$log_path" "$expected_codex_home:/codex-home:rw"
assert_file_excludes "$log_path" "$repo_dir:/workspace:rw"
assert_file_includes "$image_warning_path" "requested image command-parser:9.9.9 (sha256:parser-alt) will not take effect"

run_count="$(grep -c '^run ' "$log_path" || true)"
exec_count="$(grep -c '^exec ' "$log_path" || true)"
[[ "$run_count" == "1" ]] || fail "expected exactly one docker run call, got $run_count"
[[ "$exec_count" == "4" ]] || fail "expected exactly four docker exec calls, got $exec_count"
[[ ! -d "$fresh_codex_home" ]] || fail "expected inherited CODEX_HOME override to be ignored"

echo "PASS: command-parser container scope"

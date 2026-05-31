#!/bin/zsh

if [[ -n "${functions[strip_shim_path]:-}" ]]; then
  return 0
fi

codex_internal_shim_flag_args() {
  printf '%s\0' \
    "CODEX_SHIM_INTERNAL=1"
}

codex_exec_internal_shimmed() {
  local command_name="${1:-}"
  shift || true
  [[ -n "$command_name" ]] || return 1
  env \
    CODEX_SHIM_INTERNAL=1 \
    "$command_name" "$@"
}

codex_run_internal_shimmed_capture() {
  local __target_var="${1:-}"
  shift || true
  local command_name="${1:-}"
  shift || true
  [[ -n "$__target_var" && -n "$command_name" ]] || return 1
  local __captured=""
  __captured="$(
    env \
      CODEX_SHIM_INTERNAL=1 \
      "$command_name" "$@"
  )" || return $?
  printf -v "$__target_var" '%s' "$__captured"
}

codex_privileged_exec_output_token_limit() {
  printf '10000\n'
}

codex_emit_privileged_exec_output() {
  local stdout_file="${1:?stdout file required}"
  local stderr_file="${2:?stderr file required}"
  local exit_code="${3:?exit code required}"
  local output_token_limit=0
  local stdout_size=0
  local stderr_size=0
  local total_bytes=0
  local total_tokens=0

  output_token_limit="$(codex_privileged_exec_output_token_limit)"

  if [[ -f "$stdout_file" ]]; then
    stdout_size="$(/usr/bin/wc -c < "$stdout_file" 2>/dev/null | tr -d '[:space:]')"
  else
    stdout_size=0
  fi

  if [[ -f "$stderr_file" ]]; then
    stderr_size="$(/usr/bin/wc -c < "$stderr_file" 2>/dev/null | tr -d '[:space:]')"
  else
    stderr_size=0
  fi

  total_bytes="$((stdout_size + stderr_size))"
  total_tokens="$(((total_bytes + 3) / 4))"

  if (( total_tokens > output_token_limit )); then
    {
      printf 'Command output token limit exceed.\n'
      if [[ -n "${stdout_file}" ]]; then
        printf 'stdout: %s\n' "$stdout_file"
      fi
      if [[ -n "${stderr_file}" ]]; then
        printf 'stderr: %s\n' "$stderr_file"
      fi
    } >&2
  else
    if [[ -f "$stdout_file" ]]; then
      cat "$stdout_file"
    fi
    if [[ -f "$stderr_file" ]]; then
      cat "$stderr_file" >&2
    fi
  fi

  return "$exit_code"
}

codex_is_internal_shim_context() {
  [[ "${CODEX_SHIM_INTERNAL:-}" == "1" ]]
}

strip_shim_path() {
  local path_value="${1:-}"
  local shim_dir_legacy="/opt/homebrew/shim"
  local shim_dir_codex="$HOME/.codex/shim"
  local rebuilt=""
  local entry=""
  local -a entries=("${(@s/:/)path_value}")
  for entry in "${entries[@]}"; do
    [[ -z "$entry" || "$entry" == "$shim_dir_legacy" || "$entry" == "$shim_dir_codex" ]] && continue
    if [[ -n "$rebuilt" ]]; then
      rebuilt="${rebuilt}:$entry"
    else
      rebuilt="$entry"
    fi
  done
  printf '%s\n' "$rebuilt"
}

unshim_current_path() {
  strip_shim_path "${PATH:-}"
}

swap_home_users_prefix() {
  local path_value="${1:-}"
  case "$path_value" in
    /home/*)
      printf '/Users/%s\n' "${path_value#/home/}"
      ;;
    /Users/*)
      printf '/home/%s\n' "${path_value#/Users/}"
      ;;
    *)
      printf '%s\n' "$path_value"
      ;;
  esac
}

normalize_cwd_alias() {
  local current="${PWD:-}"
  local alternate=""
  [[ -n "$current" ]] || return 0

  if [[ ! -d "$current" ]]; then
    alternate="$(swap_home_users_prefix "$current")"
    if [[ "$alternate" != "$current" && -d "$alternate" ]]; then
      cd "$alternate"
      return 0
    fi
  fi

  case "$current" in
    /home/*|/Users/*)
      alternate="$(swap_home_users_prefix "$current")"
      if [[ "$alternate" != "$current" && -d "$alternate" ]]; then
        cd "$alternate"
      fi
      ;;
  esac
}

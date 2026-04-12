#!/bin/zsh

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

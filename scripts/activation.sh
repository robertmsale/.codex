#!/bin/zsh

ensure_minimum_runtime_path() {
  local entry=""
  for entry in /usr/bin /bin /usr/local/bin /opt/homebrew/bin; do
    case ":${PATH:-}:" in
      *:"$entry":*)
        ;;
      *)
        export PATH="${PATH:+${PATH}:}$entry"
        ;;
    esac
  done
}

source_shim_profile() {
  local profile_path="$HOME/.profile"
  [[ -f "$profile_path" ]] || return 0
  source "$profile_path" >/dev/null 2>&1 || true
}

try_activate_codex_root() {
  local activate_script="$HOME/.codex/activate"
  local codex_root="${1:-}"
  [[ -f "$activate_script" ]] || return 0
  [[ -n "$codex_root" && -d "$codex_root/skills" ]] || return 0
  source "$activate_script" "$codex_root" >/dev/null 2>&1 || true
}

activation_bridge_curl() {
  if typeset -f codex_exec_internal_shimmed >/dev/null 2>&1; then
    codex_exec_internal_shimmed curl "$@"
  else
    curl "$@"
  fi
}

resolve_agent_codex_root_from_bridge() {
  local thread_id="${CODEX_THREAD_ID:-}"
  local bridge_base_url="${ROBDEX_BRIDGE_BASE_URL:-http://127.0.0.1:42080}"
  local whoami_url=""
  local payload=""
  local project_path=""
  local cwd=""

  [[ -n "$thread_id" ]] || return 0
  command -v curl >/dev/null 2>&1 || return 0
  command -v python3 >/dev/null 2>&1 || return 0

  whoami_url="${bridge_base_url%/}/orchestrator/whoami?threadId=${thread_id}"
  if ! payload="$(activation_bridge_curl -fsS "$whoami_url" 2>/dev/null)"; then
    return 0
  fi

  if ! project_path="$(python3 -c 'import json,sys; data=json.loads(sys.stdin.read()); print(data.get("projectPath",""))' <<<"$payload" 2>/dev/null)"; then
    return 0
  fi

  [[ -n "$project_path" ]] && printf '%s/.codex\n' "$project_path" && return 0

  if ! cwd="$(python3 -c 'import json,sys; data=json.loads(sys.stdin.read()); print(data.get("cwd",""))' <<<"$payload" 2>/dev/null)"; then
    return 0
  fi

  [[ -n "$cwd" ]] || return 0
  printf '%s/.codex\n' "$cwd"
}

setup_codex_activation() {
  source_shim_profile
  try_activate_codex_root "$HOME/.codex"
  if agent_codex_root="$(resolve_agent_codex_root_from_bridge)"; then
    if [[ -n "${agent_codex_root:-}" ]]; then
      try_activate_codex_root "$agent_codex_root"
    fi
  fi
  if [[ -n "${AGENT_CWD:-}" ]]; then
    try_activate_codex_root "${AGENT_CWD}/.codex"
  fi
  ensure_minimum_runtime_path
}

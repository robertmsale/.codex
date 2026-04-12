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

resolve_agent_codex_root_from_bridge() {
  local thread_id="${CODEX_THREAD_ID:-}"
  local bridge_base_url="${ROBDEX_BRIDGE_BASE_URL:-http://127.0.0.1:42080}"
  local whoami_url=""
  local payload=""
  local cwd=""

  [[ -n "$thread_id" ]] || return 0
  command -v curl >/dev/null 2>&1 || return 0
  command -v python3 >/dev/null 2>&1 || return 0

  whoami_url="${bridge_base_url%/}/orchestrator/whoami?threadId=${thread_id}"
  if ! payload="$(curl -fsS --max-time 2 "$whoami_url" 2>/dev/null)"; then
    return 0
  fi

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

passthru_activation_prefix() {
  cat <<'EOF'
export PATH="${PATH:-}"
case ":$PATH:" in
  *:/usr/bin:*) ;;
  *) PATH="/usr/bin${PATH:+:$PATH}" ;;
esac
case ":$PATH:" in
  *:/bin:*) ;;
  *) PATH="/bin${PATH:+:$PATH}" ;;
esac
export PATH

if [[ -f "$HOME/.profile" ]]; then
  source "$HOME/.profile" >/dev/null 2>&1 || true
fi

if [[ -f "$HOME/.codex/activate" ]]; then
  source "$HOME/.codex/activate" "$HOME/.codex" >/dev/null 2>&1 || true

  if [[ -n "${CODEX_THREAD_ID:-}" ]] && command -v curl >/dev/null 2>&1 && command -v python3 >/dev/null 2>&1; then
    __robdex_bridge_base_url="${ROBDEX_BRIDGE_BASE_URL:-http://127.0.0.1:42080}"
    __robdex_payload="$(curl -fsS --max-time 2 "${__robdex_bridge_base_url%/}/orchestrator/whoami?threadId=${CODEX_THREAD_ID}" 2>/dev/null || true)"
    if [[ -n "${__robdex_payload:-}" ]]; then
      __robdex_cwd="$(python3 -c 'import json,sys; data=json.loads(sys.stdin.read()); print(data.get("cwd",""))' <<<"${__robdex_payload}" 2>/dev/null || true)"
      if [[ -n "${__robdex_cwd:-}" && -d "${__robdex_cwd}/.codex/skills" ]]; then
        source "$HOME/.codex/activate" "${__robdex_cwd}/.codex" >/dev/null 2>&1 || true
      fi
    fi
    unset __robdex_bridge_base_url __robdex_payload __robdex_cwd
  fi

  if [[ -n "${AGENT_CWD:-}" && -d "${AGENT_CWD}/.codex/skills" ]]; then
    source "$HOME/.codex/activate" "${AGENT_CWD}/.codex" >/dev/null 2>&1 || true
  fi
fi

for __codex_path_entry in /usr/bin /bin /usr/local/bin /opt/homebrew/bin; do
  case ":${PATH:-}:" in
    *:"${__codex_path_entry}":*)
      ;;
    *)
      export PATH="${PATH:+${PATH}:}${__codex_path_entry}"
      ;;
  esac
done
unset __codex_path_entry
EOF
}

#!/bin/bash
set -euo pipefail

gitops_http_base_url() {
  printf '%s\n' "${PARALLELS_SYNC_GITOPS_BASE_URL:-http://host.internal:8765}"
}

bridge_fallback_allowed() {
  [[ "${PARALLELS_SYNC_GITOPS_NO_BRIDGE:-0}" != "1" ]]
}

remove_shadow_worktree_path() {
  local worktree_path="$1"
  [[ -n "$worktree_path" ]] || return 0
  [[ -e "$worktree_path" ]] || return 0
  rm -rf "$worktree_path"
}

have_local_git_repo() {
  local path="$1"
  git -C "$path" rev-parse --git-dir >/dev/null 2>&1
}

json_escape() {
  python3 - <<'PY' "$1"
import json
import sys
print(json.dumps(sys.argv[1]))
PY
}

json_build_request() {
  python3 - <<'PY' "$@"
import json
import sys

payload = {"args": {}}
for item in sys.argv[1:]:
    key, value = item.split("=", 1)
    if value.startswith("json:"):
        payload["args"][key] = json.loads(value[5:])
    elif value in {"true", "false"}:
        payload["args"][key] = value == "true"
    else:
        payload["args"][key] = value
print(json.dumps(payload))
PY
}

http_gitops_op() {
  local op="$1"
  shift
  local payload
  payload="$(json_build_request "$@")"
  local url
  url="$(gitops_http_base_url)/v1/ops/$op"
  local response
  response="$(curl -fsS -H 'Content-Type: application/json' -d "$payload" "$url")" || {
    echo "gitops HTTP request failed for $op" >&2
    return 1
  }
  python3 - <<'PY' "$response"
import json
import sys

data = json.loads(sys.argv[1])
result = data.get("result", "")
if isinstance(result, str):
    print(result)
else:
    print(json.dumps(result))
PY
}

integration_branches() {
  printf '%s\n' "${GITOPS_INTEGRATION_BRANCHES:-main master staging prod production master}"
}

is_integration_branch() {
  local candidate="$1"
  local protected
  for protected in $(integration_branches); do
    if [[ "$candidate" == "$protected" ]]; then
      return 0
    fi
  done
  return 1
}

require_non_integration_branch() {
  local branch="$1"
  if is_integration_branch "$branch"; then
    echo "Command Rejected: You seem to be working in a restricted integration branch. Please move your file changes into a worktree and notify the user/orchestrator that the integration branch is dirty." >&2
    exit 2
  fi
}

resolve_worktree() {
  local raw_path="$1"
  local wt_abs
  wt_abs="$(cd "$raw_path" && pwd)"

  local common_abs repo_root
  common_abs="$(git -C "$wt_abs" rev-parse --path-format=absolute --git-common-dir)"
  repo_root="${common_abs%/.git}"

  if [[ "$wt_abs" == "$repo_root" ]]; then
    echo "Refusing operation on repository root; provide a linked worktree path." >&2
    exit 2
  fi

  printf '%s\n%s\n' "$wt_abs" "$repo_root"
}

resolve_worktree_or_bridge() {
  local raw_path="$1"
  if have_local_git_repo "$raw_path"; then
    resolve_worktree "$raw_path"
    return 0
  fi

  local wt_abs
  wt_abs="$(cd "$raw_path" 2>/dev/null && pwd || printf '%s\n' "$raw_path")"
  printf '%s\n%s\n' "$wt_abs" ""
}

resolve_integration_branch() {
  local repo_root="$1"
  local explicit_branch="${2:-}"
  local resolved_branch

  if [[ -n "$explicit_branch" ]]; then
    printf '%s\n' "$explicit_branch"
    return
  fi

  resolved_branch="$(git -C "$repo_root" rev-parse --abbrev-ref HEAD 2>/dev/null || true)"
  if [[ -n "$resolved_branch" && "$resolved_branch" != "HEAD" ]]; then
    printf '%s\n' "$resolved_branch"
    return
  fi

  resolved_branch="$(git -C "$repo_root" symbolic-ref --quiet --short refs/remotes/origin/HEAD 2>/dev/null | sed 's#^origin/##' || true)"
  if [[ -n "$resolved_branch" ]]; then
    printf '%s\n' "$resolved_branch"
    return
  fi

  resolved_branch="$(git -C "$repo_root" remote show origin 2>/dev/null | sed -n '/HEAD branch/s/.*: //p' | head -n 1)"
  if [[ -n "$resolved_branch" ]]; then
    printf '%s\n' "$resolved_branch"
    return
  fi

  echo "Unable to resolve integration branch for repository root: $repo_root" >&2
  exit 2
}

#!/usr/bin/env bash
set -euo pipefail

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

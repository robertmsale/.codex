#!/bin/bash
set -euo pipefail

normalize_mirrored_path() {
  local raw_path="$1"
  local host_user="${HOME##*/}"
  local normalized="$raw_path"
  case "$raw_path" in
    "/home/$host_user" | "/home/$host_user/"*)
      normalized="/Users/$host_user${raw_path#"/home/$host_user"}"
      ;;
  esac
  if [[ ! -e "$normalized" && "$normalized" == "/Users/$host_user/Code/"* ]]; then
    local without_code="/Users/$host_user/${normalized#"/Users/$host_user/Code/"}"
    if [[ -e "$without_code" ]]; then
      normalized="$without_code"
    fi
  fi
  printf '%s\n' "$normalized"
}

absolute_path() {
  local raw_path
  raw_path="$(normalize_mirrored_path "$1")"
  python3 -c 'from pathlib import Path; import sys; print(Path(sys.argv[1]).expanduser().resolve(strict=False))' "$raw_path"
}

remove_shadow_worktree_path() {
  local worktree_path="$1"
  local attempt
  worktree_path="$(normalize_mirrored_path "$worktree_path")"
  [[ -n "$worktree_path" ]] || return 0
  [[ -e "$worktree_path" ]] || return 0

  for attempt in 1 2 3 4 5 6 7 8 9 10; do
    rm -rf "$worktree_path" 2>/dev/null || true
    [[ ! -e "$worktree_path" ]] && return 0
    sleep 0.5
  done

  if [[ -e "$worktree_path" ]]; then
    echo "WARNING: shadow worktree path still exists after cleanup attempt: $worktree_path" >&2
  fi
}

protected_branches() {
  local raw="${REQUEST_REVIEW_INTEGRATION_BRANCHES:-main master staging prod production}"
  tr ', ' '\n\n' <<<"$raw" | awk 'NF'
}

is_protected_branch() {
  local branch="$1"
  local protected_branch
  while IFS= read -r protected_branch; do
    [[ -n "$protected_branch" ]] || continue
    [[ "$protected_branch" == "$branch" ]] && return 0
  done < <(protected_branches)
  return 1
}

git_subprocess_path() {
  local current_path="${PATH:-}"
  local rebuilt="/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin"
  if [[ -n "$current_path" ]]; then
    rebuilt="${rebuilt}:$current_path"
  fi
  printf '%s\n' "$rebuilt"
}

_maybe_clear_stale_index_lock() {
  local repo_path="$1"
  local stderr_file="$2"
  local stdout_file="$3"
  local combined=""
  local lock_path=""

  combined="$(cat "$stderr_file" "$stdout_file" 2>/dev/null || true)"
  [[ "$combined" == *"index.lock"* || "$combined" == *"could not write index"* ]] || return 1

  lock_path="$(python3 -c "import pathlib, re, sys; text = ''.join(pathlib.Path(path).read_text(encoding='utf-8', errors='replace') for path in sys.argv[1:]); match = re.search(r'''['\\\"]([^'\\\"]*index\\.lock)['\\\"]''', text); print(match.group(1) if match else '')" "$stderr_file" "$stdout_file")"

  if [[ -z "$lock_path" ]]; then
    lock_path="$(env PATH="$(git_subprocess_path)" git -C "$repo_path" rev-parse --path-format=absolute --git-dir 2>/dev/null)/index.lock"
  fi

  [[ -n "$lock_path" && -e "$lock_path" ]] || return 1

  if [[ -x /usr/sbin/lsof ]] && [[ -n "$(/usr/sbin/lsof "$lock_path" 2>/dev/null)" ]]; then
    return 1
  fi

  rm -f "$lock_path" || return 1
  return 0
}

clear_inactive_index_lock() {
  local repo_path="$1"
  local lock_path=""
  lock_path="$(env PATH="$(git_subprocess_path)" git -C "$repo_path" rev-parse --path-format=absolute --git-path index.lock 2>/dev/null || true)"
  [[ -n "$lock_path" && -e "$lock_path" ]] || return 0
  if [[ -x /usr/sbin/lsof ]] && [[ -n "$(/usr/sbin/lsof "$lock_path" 2>/dev/null)" ]]; then
    echo "Refusing to clear active git index lock: $lock_path" >&2
    return 1
  fi
  rm -f "$lock_path"
}

_safe_move_path() {
  local path="$1"
  local base dest name stamp
  [[ -e "$path" ]] || return 0
  base="/tmp/safe-delete"
  mkdir -p "$base"
  name="$(basename "$path")"
  stamp="$(date +%Y%m%d%H%M%S)"
  dest="$base/${name}-${stamp}-$$"
  mv "$path" "$dest"
  printf 'moved %s -> %s\n' "$path" "$dest" >&2
}

clean_known_qa_generated_artifacts() {
  local repo_path="$1"
  local pathspec="clients/packages/design_system/lib/src/copy/generated"
  local status line code path
  status="$(env PATH="$(git_subprocess_path)" git -C "$repo_path" status --porcelain=v1 -- "$pathspec" 2>/dev/null || true)"
  [[ -n "$status" ]] || return 0
  while IFS= read -r line; do
    [[ -n "$line" ]] || continue
    code="${line:0:2}"
    path="${line:3}"
    if [[ "$code" == "??" && "$path" == "$pathspec"* ]]; then
      _safe_move_path "$repo_path/$path"
    fi
  done <<<"$status"
}

run_checked() {
  local cwd="$1"
  shift
  local stdout_file stderr_file rc
  stdout_file="$(mktemp)"
  stderr_file="$(mktemp)"
  rc=0
  env PATH="$(git_subprocess_path)" "$@" >"$stdout_file" 2>"$stderr_file" || rc=$?
  if [[ "$rc" -ne 0 ]]; then
    if [[ "${1:-}" == "git" ]] && _maybe_clear_stale_index_lock "$cwd" "$stderr_file" "$stdout_file"; then
      rc=0
      env PATH="$(git_subprocess_path)" "$@" >"$stdout_file" 2>"$stderr_file" || rc=$?
    fi
  fi
  if [[ "$rc" -ne 0 ]]; then
    local detail
    detail="$(python3 -c 'import pathlib, sys; stderr = pathlib.Path(sys.argv[1]).read_text(encoding="utf-8", errors="replace").strip(); stdout = pathlib.Path(sys.argv[2]).read_text(encoding="utf-8", errors="replace").strip(); print(stderr or stdout or "command failed")' "$stderr_file" "$stdout_file")"
    rm -f "$stdout_file" "$stderr_file"
    echo "$detail" >&2
    return "$rc"
  fi
  cat "$stdout_file"
  rm -f "$stdout_file" "$stderr_file"
}

run_checked_with_env() {
  local cwd="$1"
  local env_prefix="$2"
  shift 2
  local stdout_file stderr_file rc
  stdout_file="$(mktemp)"
  stderr_file="$(mktemp)"
  rc=0
  env PATH="$(git_subprocess_path)" $env_prefix "$@" >"$stdout_file" 2>"$stderr_file" || rc=$?
  if [[ "$rc" -ne 0 ]]; then
    if [[ "${1:-}" == "git" ]] && _maybe_clear_stale_index_lock "$cwd" "$stderr_file" "$stdout_file"; then
      rc=0
      env PATH="$(git_subprocess_path)" $env_prefix "$@" >"$stdout_file" 2>"$stderr_file" || rc=$?
    fi
  fi
  if [[ "$rc" -ne 0 ]]; then
    local detail
    detail="$(python3 -c 'import pathlib, sys; stderr = pathlib.Path(sys.argv[1]).read_text(encoding="utf-8", errors="replace").strip(); stdout = pathlib.Path(sys.argv[2]).read_text(encoding="utf-8", errors="replace").strip(); print(stderr or stdout or "command failed")' "$stderr_file" "$stdout_file")"
    rm -f "$stdout_file" "$stderr_file"
    echo "$detail" >&2
    return "$rc"
  fi
  cat "$stdout_file"
  rm -f "$stdout_file" "$stderr_file"
}

require_local_git_repo() {
  local repo_path
  repo_path="$(absolute_path "$1")"
  if ! env PATH="$(git_subprocess_path)" git -C "$repo_path" rev-parse --git-dir >/dev/null 2>&1; then
    echo "Path is not a valid local git repository: $repo_path" >&2
    return 1
  fi
  printf '%s\n' "$repo_path"
}

current_branch() {
  local repo_path="$1"
  run_checked "$repo_path" git -C "$repo_path" rev-parse --abbrev-ref HEAD | tr -d '\n'
}

ensure_branch_allows_destructive_mutation() {
  local repo_path="$1"
  local branch
  branch="$(current_branch "$repo_path")"
  if is_protected_branch "$branch"; then
    echo "Refusing destructive mutation on protected integration branch '$branch'. Only additive mutations and abort flows are allowed." >&2
    return 1
  fi
  printf '%s\n' "$branch"
}

worktree_checkout_root() {
  local repo_path
  repo_path="$(absolute_path "$1")"
  run_checked "$repo_path" git -C "$repo_path" rev-parse --path-format=absolute --show-toplevel | tr -d '\n'
}

worktree_repo_root() {
  local repo_path checkout_root common_dir
  repo_path="$(absolute_path "$1")"
  checkout_root="$(worktree_checkout_root "$repo_path")"
  common_dir="$(run_checked "$checkout_root" git -C "$checkout_root" rev-parse --path-format=absolute --git-common-dir | tr -d '\n')"
  python3 -c 'from pathlib import Path; import sys; print(Path(sys.argv[1]).resolve().parent)' "$common_dir"
}

REQUIRED_WORKTREE_ROOT=""
REQUIRED_REPO_ROOT=""

require_managed_worktree_path() {
  local worktree_path checkout_root repo_root managed_root
  worktree_path="$(absolute_path "$1")"
  checkout_root="$(worktree_checkout_root "$worktree_path")"
  repo_root="$(worktree_repo_root "$checkout_root")"
  if [[ "$checkout_root" == "$repo_root" ]]; then
    echo "Refusing to mutate checked-out base repo $repo_root. Use a dedicated worktree under .worktrees/ instead." >&2
    return 1
  fi
  managed_root="$(python3 -c 'from pathlib import Path; import sys; print((Path(sys.argv[1]).resolve() / ".worktrees").resolve())' "$repo_root")"
  if ! python3 -c 'from pathlib import Path; import sys; managed = Path(sys.argv[1]).resolve(); checkout = Path(sys.argv[2]).resolve(); raise SystemExit(0 if managed in checkout.parents else 1)' "$managed_root" "$checkout_root"
  then
    echo "Refusing unmanaged worktree path $checkout_root. Use a dedicated worktree under $managed_root." >&2
    return 1
  fi
  REQUIRED_WORKTREE_ROOT="$checkout_root"
  REQUIRED_REPO_ROOT="$repo_root"
}

resolve_integration_branch() {
  local repo_root explicit_branch output
  repo_root="$(absolute_path "$1")"
  explicit_branch="${2:-}"
  if [[ -n "${explicit_branch// /}" ]]; then
    printf '%s\n' "${explicit_branch//[[:space:]]/}"
    return 0
  fi

  output="$(env PATH="$(git_subprocess_path)" git -C "$repo_root" symbolic-ref --quiet --short refs/remotes/origin/HEAD 2>/dev/null || true)"
  if [[ -n "$output" && "$output" != "HEAD" ]]; then
    printf '%s\n' "${output#origin/}"
    return 0
  fi

  output="$(env PATH="$(git_subprocess_path)" git -C "$repo_root" remote show origin 2>/dev/null || true)"
  if [[ -n "$output" ]]; then
    if python3 -c 'import sys; 
for line in sys.argv[1].splitlines():
    if "HEAD branch:" in line:
        print(line.split("HEAD branch:", 1)[1].strip())
        raise SystemExit(0)
raise SystemExit(1)' "$output"
    then
      return 0
    fi
  fi

  printf '%s\n' "${GITOPS_INTEGRATION_BRANCH:-master}"
}

maybe_stash() {
  local repo_path="$1"
  local label="$2"
  local status stash_name
  status="$(run_checked "$repo_path" git -C "$repo_path" status --short | tr -d '\r')"
  if [[ -z "$status" ]]; then
    printf '\n'
    return 0
  fi
  stash_name="${label}-$(date +%s)"
  run_checked "$repo_path" git -C "$repo_path" stash push -u -m "$stash_name" >/dev/null
  printf '%s\n' "$stash_name"
}

restore_stash() {
  local repo_path="$1"
  local stash_name="${2:-}"
  local stash_ref=""
  local stash_list line
  [[ -n "$stash_name" ]] || return 0
  stash_list="$(run_checked "$repo_path" git -C "$repo_path" stash list --format=%gd\ %s)"
  while IFS= read -r line; do
    [[ "$line" == *"$stash_name"* ]] || continue
    stash_ref="${line%% *}"
    break
  done <<<"$stash_list"
  if [[ -z "$stash_ref" ]]; then
    printf 'stash %s was preserved for manual recovery\n' "$stash_name"
    return 0
  fi
  if env PATH="$(git_subprocess_path)" git -C "$repo_path" stash apply "$stash_ref" >/dev/null 2>&1; then
    run_checked "$repo_path" git -C "$repo_path" stash drop "$stash_ref" >/dev/null
    printf 'restored stash %s\n' "$stash_name"
    return 0
  fi
  printf 'stash %s was preserved for manual recovery\n' "$stash_name"
}

git_status_short_branch() {
  local repo_path
  repo_path="$(require_local_git_repo "$1")"
  run_checked "$repo_path" git -C "$repo_path" status --short --branch
}

git_branch_list() {
  local repo_path all_flag
  repo_path="$(require_local_git_repo "$1")"
  all_flag="${2:-true}"
  if [[ "$all_flag" == "true" ]]; then
    run_checked "$repo_path" git -C "$repo_path" branch --all --verbose --verbose
  else
    run_checked "$repo_path" git -C "$repo_path" branch --verbose --verbose
  fi
}

git_diff_show() {
  local repo_path ref pathspec
  repo_path="$(require_local_git_repo "$1")"
  ref="${2:-}"
  pathspec="${3:-}"
  local cmd=(git -C "$repo_path" diff)
  [[ -n "$ref" ]] && cmd+=("$ref")
  [[ -n "$pathspec" ]] && cmd+=(-- "$pathspec")
  run_checked "$repo_path" "${cmd[@]}"
}

git_show_object() {
  local repo_path object
  repo_path="$(require_local_git_repo "$1")"
  object="$2"
  run_checked "$repo_path" git -C "$repo_path" show --stat "$object"
}

git_rev_parse_verify() {
  local repo_path ref
  repo_path="$(require_local_git_repo "$1")"
  ref="${2:-HEAD}"
  run_checked "$repo_path" git -C "$repo_path" rev-parse --verify "$ref"
}

git_merge_base_refs() {
  local repo_path left right
  repo_path="$(require_local_git_repo "$1")"
  left="$2"
  right="$3"
  run_checked "$repo_path" git -C "$repo_path" merge-base "$left" "$right"
}

git_stage_paths_cmd() {
  local repo_path="$1"
  shift
  repo_path="$(absolute_path "$repo_path")"
  [[ $# -gt 0 ]] || { echo "paths must be a non-empty list" >&2; return 1; }
  run_checked "$repo_path" git -C "$repo_path" add -- "$@"
}

git_unstage_paths_cmd() {
  local repo_path="$1"
  shift
  repo_path="$(absolute_path "$repo_path")"
  ensure_branch_allows_destructive_mutation "$repo_path" >/dev/null
  [[ $# -gt 0 ]] || { echo "paths must be a non-empty list" >&2; return 1; }
  run_checked "$repo_path" git -C "$repo_path" reset HEAD -- "$@"
}

git_rebase_abort_cmd() {
  local repo_path
  repo_path="$(absolute_path "$1")"
  current_branch "$repo_path" >/dev/null
  run_checked "$repo_path" git -C "$repo_path" rebase --abort
}

git_rebase_continue_cmd() {
  local repo_path
  repo_path="$(absolute_path "$1")"
  run_checked_with_env "$repo_path" "GIT_EDITOR=true EDITOR=true VISUAL=true" git -C "$repo_path" rebase --continue
}

git_worktree_root() {
  local repo_path
  repo_path="$(absolute_path "$1")"
  printf '%s\n' "$repo_path/.worktrees"
}

git_worktree_create_cmd() {
  local repo_path base_branch branch_name worktree_name worktree_root worktree_path
  local stdout_file stderr_file rc detail
  repo_path="$(require_local_git_repo "$1")"
  base_branch="$2"
  branch_name="$3"
  worktree_name="$4"
  [[ -n "$base_branch" && -n "$branch_name" && -n "$worktree_name" ]] || {
    echo "repo_path, base_branch, branch_name, and worktree_name are required" >&2
    return 1
  }
  if is_protected_branch "$branch_name"; then
    echo "Refusing to create a managed worktree on protected integration branch '$branch_name'." >&2
    return 1
  fi
  worktree_root="$(git_worktree_root "$repo_path")"
  mkdir -p "$worktree_root"
  worktree_path="$(absolute_path "$worktree_root/$worktree_name")"
  [[ ! -e "$worktree_path" ]] || { echo "Worktree path already exists: $worktree_path" >&2; return 1; }
  run_checked "$repo_path" git -C "$repo_path" fetch -q origin --prune >/dev/null
  run_checked "$repo_path" git -C "$repo_path" show-ref --verify --quiet "refs/remotes/origin/$base_branch" >/dev/null
  if env PATH="$(git_subprocess_path)" git -C "$repo_path" show-ref --verify --quiet "refs/heads/$branch_name" >/dev/null 2>&1; then
    echo "Local branch already exists: $branch_name" >&2
    return 1
  fi
  stdout_file="$(mktemp)"
  stderr_file="$(mktemp)"
  rc=0
  env PATH="$(git_subprocess_path)" git -C "$repo_path" worktree add -b "$branch_name" "$worktree_path" "origin/$base_branch" >"$stdout_file" 2>"$stderr_file" || rc=$?
  if [[ "$rc" -ne 0 ]]; then
    detail="$(python3 -c 'import pathlib, sys; stderr = pathlib.Path(sys.argv[1]).read_text(encoding="utf-8", errors="replace").strip(); stdout = pathlib.Path(sys.argv[2]).read_text(encoding="utf-8", errors="replace").strip(); print(stderr or stdout or "command failed")' "$stderr_file" "$stdout_file")"
    if [[ "$detail" == *"could not lock config file .git/config"* || "$detail" == *"unable to write upstream branch configuration"* ]]; then
      if [[ -d "$worktree_path" ]] \
        && env PATH="$(git_subprocess_path)" git -C "$repo_path" show-ref --verify --quiet "refs/heads/$branch_name" >/dev/null 2>&1 \
        && [[ "$(env PATH="$(git_subprocess_path)" git -C "$worktree_path" rev-parse --abbrev-ref HEAD 2>/dev/null | tr -d '\n')" == "$branch_name" ]]
      then
        rc=0
      fi
    fi
  fi
  rm -f "$stdout_file" "$stderr_file"
  if [[ "$rc" -ne 0 ]]; then
    echo "${detail:-command failed}" >&2
    return "$rc"
  fi
  printf 'created worktree %s on branch %s from origin/%s\n' "$worktree_path" "$branch_name" "$base_branch"
}

git_worktree_refresh_branch_cmd() {
  local worktree_path new_branch integration_branch current_branch
  worktree_path="$(absolute_path "$1")"
  new_branch="$2"
  integration_branch="$3"
  [[ -n "${new_branch// /}" ]] || { echo "new_branch is required" >&2; return 1; }
  if is_protected_branch "$new_branch"; then
    echo "Refusing to check out protected integration branch '$new_branch' in a managed worktree." >&2
    return 1
  fi
  require_managed_worktree_path "$worktree_path"
  integration_branch="$(resolve_integration_branch "$REQUIRED_REPO_ROOT" "$integration_branch")"
  [[ -z "$(run_checked "$REQUIRED_WORKTREE_ROOT" git -C "$REQUIRED_WORKTREE_ROOT" status --short | tr -d '\r')" ]] || {
    echo "Refusing to refresh a dirty worktree: $REQUIRED_WORKTREE_ROOT" >&2
    return 1
  }
  run_checked "$REQUIRED_WORKTREE_ROOT" git -C "$REQUIRED_WORKTREE_ROOT" fetch -q origin --prune >/dev/null
  run_checked "$REQUIRED_WORKTREE_ROOT" git -C "$REQUIRED_WORKTREE_ROOT" show-ref --verify --quiet "refs/remotes/origin/$integration_branch" >/dev/null
  if env PATH="$(git_subprocess_path)" git -C "$REQUIRED_REPO_ROOT" show-ref --verify --quiet "refs/heads/$new_branch" >/dev/null 2>&1; then
    echo "Local branch already exists: $new_branch" >&2
    return 1
  fi
  if env PATH="$(git_subprocess_path)" git -C "$REQUIRED_REPO_ROOT" show-ref --verify --quiet "refs/remotes/origin/$new_branch" >/dev/null 2>&1; then
    echo "Remote branch already exists: origin/$new_branch" >&2
    return 1
  fi
  current_branch="$(current_branch "$REQUIRED_WORKTREE_ROOT")"
  run_checked "$REQUIRED_WORKTREE_ROOT" git -C "$REQUIRED_WORKTREE_ROOT" switch --create "$new_branch" "origin/$integration_branch" >/dev/null
  printf 'switched %s from %s to fresh branch %s at origin/%s\n' "$REQUIRED_WORKTREE_ROOT" "$current_branch" "$new_branch" "$integration_branch"
}

git_worktree_cleanup_cmd() {
  local worktree_path branch checked_out_elsewhere attempt remove_rc
  worktree_path="$(absolute_path "$1")"
  if [[ ! -e "$worktree_path" ]]; then
    printf 'all clear: worktree path is already missing\n'
    return 0
  fi
  require_managed_worktree_path "$worktree_path"
  branch="$(current_branch "$REQUIRED_WORKTREE_ROOT")"
  remove_rc=0
  for attempt in 1 2 3 4 5 6; do
    remove_rc=0
    env PATH="$(git_subprocess_path)" git -C "$REQUIRED_REPO_ROOT" worktree remove --force "$REQUIRED_WORKTREE_ROOT" >/dev/null 2>&1 || remove_rc=$?
    if [[ "$remove_rc" -eq 0 ]] || [[ ! -e "$REQUIRED_WORKTREE_ROOT" ]]; then
      remove_rc=0
      break
    fi
    sleep 1
  done
  if [[ "$remove_rc" -ne 0 ]]; then
    run_checked "$REQUIRED_REPO_ROOT" git -C "$REQUIRED_REPO_ROOT" worktree remove --force "$REQUIRED_WORKTREE_ROOT" >/dev/null
  fi
  checked_out_elsewhere="$(env PATH="$(git_subprocess_path)" git -C "$REQUIRED_REPO_ROOT" worktree list --porcelain 2>/dev/null || true)"
  if [[ "$checked_out_elsewhere" != *"branch refs/heads/$branch"* ]]; then
    env PATH="$(git_subprocess_path)" git -C "$REQUIRED_REPO_ROOT" branch -D "$branch" >/dev/null 2>&1 || true
  fi
  run_checked "$REQUIRED_REPO_ROOT" git -C "$REQUIRED_REPO_ROOT" worktree prune >/dev/null
  remove_shadow_worktree_path "$worktree_path"
  printf 'removed worktree %s and pruned metadata\n' "$REQUIRED_WORKTREE_ROOT"
}

fastforward_local_integration_branch_cmd() {
  local repo_root integration_branch current_branch checked_out_elsewhere stash_name restore_message
  repo_root="$(require_local_git_repo "$1")"
  integration_branch="$2"
  [[ -n "$integration_branch" ]] || { echo "integration_branch is required" >&2; return 1; }

  run_checked "$repo_root" git -C "$repo_root" fetch -q origin --prune >/dev/null
  run_checked "$repo_root" git -C "$repo_root" show-ref --verify --quiet "refs/remotes/origin/$integration_branch" >/dev/null

  current_branch="$(current_branch "$repo_root")"
  if [[ "$current_branch" == "$integration_branch" ]]; then
    stash_name="$(maybe_stash "$repo_root" "git-merge-worktree-$integration_branch")"
    run_checked "$repo_root" git -C "$repo_root" merge --ff-only "origin/$integration_branch" >/dev/null
    if [[ -n "$stash_name" ]]; then
      restore_message="$(restore_stash "$repo_root" "$stash_name")"
      printf '%s\n' "$restore_message" >&2
    fi
    return 0
  fi

  checked_out_elsewhere="$(env PATH="$(git_subprocess_path)" git -C "$repo_root" worktree list --porcelain 2>/dev/null || true)"
  if [[ "$checked_out_elsewhere" == *"branch refs/heads/$integration_branch"* ]]; then
    echo "Integration branch '$integration_branch' is checked out in another worktree; cannot fast-forward the local branch ref here." >&2
    return 1
  fi

  if env PATH="$(git_subprocess_path)" git -C "$repo_root" show-ref --verify --quiet "refs/heads/$integration_branch" >/dev/null 2>&1; then
    run_checked "$repo_root" git -C "$repo_root" branch -f "$integration_branch" "origin/$integration_branch" >/dev/null
  else
    run_checked "$repo_root" git -C "$repo_root" branch --track "$integration_branch" "origin/$integration_branch" >/dev/null
  fi
}

git_sync_worktree_cmd() {
  local worktree_path upstream branch stash_name restore_message
  worktree_path="$(absolute_path "$1")"
  upstream="$2"
  [[ -n "$upstream" ]] || { echo "upstream is required" >&2; return 1; }
  require_managed_worktree_path "$worktree_path"
  branch="$(current_branch "$REQUIRED_WORKTREE_ROOT")"
  stash_name="$(maybe_stash "$REQUIRED_WORKTREE_ROOT" "git-sync-$branch")"
  run_checked "$REQUIRED_WORKTREE_ROOT" git -C "$REQUIRED_WORKTREE_ROOT" fetch -q origin --prune >/dev/null
  run_checked_with_env "$REQUIRED_WORKTREE_ROOT" "GIT_EDITOR=true EDITOR=true VISUAL=true" git -C "$REQUIRED_WORKTREE_ROOT" rebase "$upstream" >/dev/null
  if [[ -n "$stash_name" ]]; then
    restore_message="$(restore_stash "$REQUIRED_WORKTREE_ROOT" "$stash_name")"
    printf 'rebased %s onto %s; %s\n' "$branch" "$upstream" "$restore_message"
  else
    printf 'rebased %s onto %s\n' "$branch" "$upstream"
  fi
}

git_recover_published_worktree_cmd() {
  local worktree_path integration_branch
  worktree_path="$(absolute_path "$1")"
  integration_branch="${2:-}"
  integration_branch="$(resolve_integration_branch "$worktree_path" "$integration_branch")"

  if git_sync_worktree_cmd "$worktree_path" "origin/$integration_branch"; then
    printf 'recovered published worktree %s in place on its existing PR branch. rerun proof, then publish with git-publish-worktree %s %s\n' \
      "$worktree_path" "$worktree_path" "$integration_branch"
    return 0
  fi

  if env PATH="$(git_subprocess_path)" git -C "$worktree_path" rev-parse --git-path rebase-merge >/dev/null 2>&1; then
    local rebase_merge_path=""
    local rebase_apply_path=""
    rebase_merge_path="$(env PATH="$(git_subprocess_path)" git -C "$worktree_path" rev-parse --git-path rebase-merge 2>/dev/null || true)"
    rebase_apply_path="$(env PATH="$(git_subprocess_path)" git -C "$worktree_path" rev-parse --git-path rebase-apply 2>/dev/null || true)"
    if [[ -d "$rebase_merge_path" || -d "$rebase_apply_path" ]]; then
      cat >&2 <<EOF
Published PR branch recovery is in progress in this same worktree.
Stay on the existing branch/worktree.
Resolve conflicts here, stage the resolutions, then run:
  git-rebase-continue $worktree_path
After the rebase finishes:
  rerun proof in this same worktree
  git-publish-worktree $worktree_path $integration_branch
Do not create a new branch or cherry-pick the old commit unless explicitly directed.
EOF
    fi
  fi
  return 1
}

git_fetch_cmd() {
  local repo_path remote
  repo_path="$(require_local_git_repo "$1")"
  remote="${2:-origin}"
  run_checked "$repo_path" git -C "$repo_path" fetch -q "$remote" --prune >/dev/null
  printf 'fetched %s and pruned remote-tracking refs\n' "$remote"
}

qa_fastforward_cmd() {
  local worktree_path checkout_root repo_root branch target_branch stash_name local_head remote_head stash_list stash_ref line
  worktree_path="$(absolute_path "$1")"
  checkout_root="$(worktree_checkout_root "$worktree_path")"
  repo_root="$(worktree_repo_root "$checkout_root")"
  branch="$(current_branch "$checkout_root")"
  target_branch="$(resolve_integration_branch "$repo_root" "${2:-}")"

  if [[ "$checkout_root" == "$repo_root" ]]; then
    if [[ "$branch" != "$target_branch" ]]; then
      echo "QA fast-forward against base repo requires the checked-out integration branch. Current branch is $branch, target is $target_branch." >&2
      return 1
    fi
    run_checked "$checkout_root" git -C "$checkout_root" fetch -q origin --prune >/dev/null
    run_checked "$checkout_root" git -C "$checkout_root" merge --ff-only "origin/$target_branch" >/dev/null
    printf 'Fast-forwarded checked-out %s to origin/%s\n' "$branch" "$target_branch"
    return 0
  fi

  require_managed_worktree_path "$checkout_root"
  stash_name=""
  clear_inactive_index_lock "$REQUIRED_WORKTREE_ROOT"
  clean_known_qa_generated_artifacts "$REQUIRED_WORKTREE_ROOT"
  if [[ -n "$(run_checked "$REQUIRED_WORKTREE_ROOT" git -C "$REQUIRED_WORKTREE_ROOT" status --short | tr -d '\r')" ]]; then
    stash_name="qa-fastforward-${branch}-$(date +%s)"
    run_checked "$REQUIRED_WORKTREE_ROOT" git -C "$REQUIRED_WORKTREE_ROOT" stash push -u -m "$stash_name" >/dev/null
  fi
  run_checked "$REQUIRED_WORKTREE_ROOT" git -C "$REQUIRED_WORKTREE_ROOT" fetch -q origin --prune >/dev/null
  local_head="$(run_checked "$REQUIRED_WORKTREE_ROOT" git -C "$REQUIRED_WORKTREE_ROOT" rev-parse HEAD | tr -d '\n')"
  remote_head="$(run_checked "$REQUIRED_WORKTREE_ROOT" git -C "$REQUIRED_WORKTREE_ROOT" rev-parse "origin/$target_branch" | tr -d '\n')"
  if [[ "$local_head" != "$remote_head" ]]; then
    if is_protected_branch "$branch"; then
      run_checked "$REQUIRED_WORKTREE_ROOT" git -C "$REQUIRED_WORKTREE_ROOT" merge --ff-only "origin/$target_branch" >/dev/null
    else
      run_checked "$REQUIRED_WORKTREE_ROOT" git -C "$REQUIRED_WORKTREE_ROOT" rebase "origin/$target_branch" >/dev/null
    fi
  fi
  if [[ -n "$stash_name" ]]; then
    stash_list="$(run_checked "$REQUIRED_REPO_ROOT" git -C "$REQUIRED_REPO_ROOT" stash list --format=%gd\ %s)"
    stash_ref=""
    while IFS= read -r line; do
      [[ "$line" == *"$stash_name"* ]] || continue
      stash_ref="${line%% *}"
      break
    done <<<"$stash_list"
    if [[ -n "$stash_ref" ]] && env PATH="$(git_subprocess_path)" git -C "$REQUIRED_WORKTREE_ROOT" stash apply "$stash_ref" >/dev/null 2>&1; then
      run_checked "$REQUIRED_WORKTREE_ROOT" git -C "$REQUIRED_WORKTREE_ROOT" stash drop "$stash_ref" >/dev/null
      printf 'Fast-forwarded %s onto origin/%s and restored stash %s\n' "$branch" "$target_branch" "$stash_name"
    else
      printf 'Fast-forwarded %s onto origin/%s; stash %s was preserved for manual recovery\n' "$branch" "$target_branch" "$stash_name"
    fi
  else
    printf 'Fast-forwarded %s onto origin/%s\n' "$branch" "$target_branch"
  fi
}

git_commit_cmd() {
  local worktree_path message allow_empty add_all
  worktree_path="$(absolute_path "$1")"
  message="$2"
  allow_empty="${3:-false}"
  add_all="${4:-true}"
  [[ -n "${message// /}" ]] || { echo "message is required" >&2; return 1; }
  require_managed_worktree_path "$worktree_path"
  if [[ "$add_all" == "true" ]]; then
    run_checked "$REQUIRED_WORKTREE_ROOT" git -C "$REQUIRED_WORKTREE_ROOT" add -A >/dev/null
  fi
  local commit_cmd=(git -C "$REQUIRED_WORKTREE_ROOT" commit -m "$message")
  [[ "$allow_empty" == "true" ]] && commit_cmd+=(--allow-empty)
  run_checked_with_env "$REQUIRED_WORKTREE_ROOT" "GIT_EDITOR=true EDITOR=true VISUAL=true" "${commit_cmd[@]}" >/dev/null
  printf 'committed in %s: %s\n' "$REQUIRED_WORKTREE_ROOT" "$message"
}

git_publish_worktree_cmd() {
  local worktree_path integration_branch branch pr_json pr_number pr_url pr_state pr_draft pr_head pr_base pr_title latest_title latest_body
  worktree_path="$(absolute_path "$1")"
  integration_branch="${2:-}"
  [[ -d "$worktree_path" ]] || { echo "Worktree path does not exist: $worktree_path" >&2; return 1; }
  [[ -f "$worktree_path/review.log" ]] || { echo "Publish blocked: review.log not found in worktree root. Request review first." >&2; return 1; }
  require_managed_worktree_path "$worktree_path"
  integration_branch="$(resolve_integration_branch "$REQUIRED_REPO_ROOT" "$integration_branch")"
  branch="$(ensure_branch_allows_destructive_mutation "$REQUIRED_WORKTREE_ROOT")"
  [[ -z "$(run_checked "$REQUIRED_WORKTREE_ROOT" git -C "$REQUIRED_WORKTREE_ROOT" status --short | tr -d '\r')" ]] || {
    echo "Refusing to publish a dirty worktree: $REQUIRED_WORKTREE_ROOT" >&2
    return 1
  }

  if ! env PATH="$(git_subprocess_path)" git -C "$REQUIRED_WORKTREE_ROOT" push -q --set-upstream origin "$branch" >/dev/null 2>&1; then
    run_checked "$REQUIRED_WORKTREE_ROOT" git -C "$REQUIRED_WORKTREE_ROOT" push -q --force-with-lease --set-upstream origin "$branch" >/dev/null
  fi

  pr_json="$(env PATH="$(git_subprocess_path)" gh pr view "$branch" --json number,url,state,isDraft,headRefName,baseRefName,title 2>/dev/null || true)"
  if [[ -z "$pr_json" ]]; then
    if ! run_checked "$REQUIRED_WORKTREE_ROOT" gh pr create --head "$branch" --base "$integration_branch" --fill >/dev/null; then
      latest_title="$(run_checked "$REQUIRED_WORKTREE_ROOT" git -C "$REQUIRED_WORKTREE_ROOT" log -1 --pretty=%s | tr -d '\n')"
      latest_body="$(run_checked "$REQUIRED_WORKTREE_ROOT" git -C "$REQUIRED_WORKTREE_ROOT" log -1 --pretty=%b)"
      if [[ -z "${latest_body// /}" ]]; then
        latest_body="$latest_title"
      fi
      run_checked "$REQUIRED_WORKTREE_ROOT" gh pr create --head "$branch" --base "$integration_branch" --title "$latest_title" --body "$latest_body" >/dev/null
    fi
    pr_json="$(run_checked "$REQUIRED_WORKTREE_ROOT" gh pr view "$branch" --json number,url,state,isDraft,headRefName,baseRefName,title)"
  fi

  read -r pr_number pr_url pr_state pr_draft pr_head pr_base pr_title < <(python3 -c 'import json, sys; data = json.loads(sys.argv[1] or "{}"); values = [str(data.get("number") or ""), str(data.get("url") or "no-url"), str(data.get("state") or "unknown"), str(data.get("isDraft")), str(data.get("headRefName") or ""), str(data.get("baseRefName") or ""), str(data.get("title") or "")]; print("\t".join(values))' "$pr_json")
  run_checked "$REQUIRED_WORKTREE_ROOT" git -C "$REQUIRED_WORKTREE_ROOT" fetch -q origin --prune >/dev/null
  printf 'published branch %s to %s; PR #%s (%s) state=%s draft=%s title=%s. refreshed remote-tracking refs.\n' \
    "${pr_head:-$branch}" "${pr_base:-$integration_branch}" "$pr_number" "$pr_url" "$pr_state" "$pr_draft" "$pr_title"
}

git_merge_worktree_cmd() {
  local worktree_path integration_branch branch pr_number cleanup_text cleanup_warning ls_remote_status show_ref_status completed_steps=""
  worktree_path="$(absolute_path "$1")"
  integration_branch="${2:-}"
  require_managed_worktree_path "$worktree_path"
  integration_branch="$(resolve_integration_branch "$REQUIRED_REPO_ROOT" "$integration_branch")"
  branch="$(ensure_branch_allows_destructive_mutation "$REQUIRED_WORKTREE_ROOT")"
  [[ -z "$(run_checked "$REQUIRED_WORKTREE_ROOT" git -C "$REQUIRED_WORKTREE_ROOT" status --short | tr -d '\r')" ]] || {
    echo "Refusing to merge a dirty worktree: $REQUIRED_WORKTREE_ROOT" >&2
    return 1
  }
  pr_number="$(run_checked "$REQUIRED_WORKTREE_ROOT" gh pr view "$branch" --json number --jq .number | tr -d '\n')"
  [[ -n "$pr_number" ]] || { echo "No PR found for worktree branch: $branch" >&2; return 1; }

  if ! env PATH="$(git_subprocess_path)" gh pr merge "$pr_number" --squash >/dev/null 2>&1; then
    cat >&2 <<EOF
Published PR branch is not mergeable right now.
Stay on this same branch/worktree and recover it in place:
  git-recover-published-worktree $REQUIRED_WORKTREE_ROOT $integration_branch
If the rebase stops on conflicts:
  resolve conflicts in this same worktree
  git-rebase-continue $REQUIRED_WORKTREE_ROOT
Then rerun proof and publish the same PR branch:
  git-publish-worktree $REQUIRED_WORKTREE_ROOT $integration_branch
Do not create a new branch/worktree or cherry-pick the old commit unless explicitly directed.
EOF
    return 1
  fi
  completed_steps="squash-merged PR #$pr_number"

  if env PATH="$(git_subprocess_path)" git -C "$REQUIRED_REPO_ROOT" ls-remote --exit-code --heads origin "$branch" >/dev/null 2>&1; then
    run_checked "$REQUIRED_REPO_ROOT" git -C "$REQUIRED_REPO_ROOT" push -q origin --delete "$branch" >/dev/null
    completed_steps="$completed_steps. deleted origin/$branch"
  fi

  run_checked "$REQUIRED_REPO_ROOT" git -C "$REQUIRED_REPO_ROOT" fetch -q origin --prune >/dev/null
  completed_steps="$completed_steps. pruned remote refs"
  fastforward_local_integration_branch_cmd "$REQUIRED_REPO_ROOT" "$integration_branch"
  completed_steps="$completed_steps. fast-forwarded local $integration_branch to origin/$integration_branch"

  if cleanup_text="$(git_worktree_cleanup_cmd "$REQUIRED_WORKTREE_ROOT")"; then
    completed_steps="$completed_steps. $cleanup_text"
  else
    cleanup_warning=" WARNING: worktree cleanup failed"
    printf '%s.%s\n' "$completed_steps" "$cleanup_warning"
    return 0
  fi
  printf '%s.\n' "$completed_steps"
}

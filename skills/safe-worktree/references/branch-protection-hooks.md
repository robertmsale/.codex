# Branch Protection Hooks (Reference)

Use these hook templates when a repository does not already protect integration branches from deletion.

Protected branch baseline:
- `main`
- `master`
- `staging`
- `prod`

## pre-push
Purpose:
- Prevent deleting protected branches on the remote (`git push origin --delete <branch>`).

Template:

```bash
#!/usr/bin/env bash
set -euo pipefail

protected_branches=("main" "master" "staging" "prod")

is_protected_ref() {
  local ref="$1"
  local branch
  for branch in "${protected_branches[@]}"; do
    if [[ "$ref" == "refs/heads/$branch" ]]; then
      return 0
    fi
  done
  return 1
}

# Input per line:
# <local-ref> <local-object-name> <remote-ref> <remote-object-name>
while read -r local_ref _local_sha remote_ref _remote_sha; do
  [[ -z "${local_ref:-}" ]] && continue

  if [[ "$local_ref" == "(delete)" ]] && is_protected_ref "$remote_ref"; then
    echo "pre-push: refusing to delete protected remote branch: ${remote_ref#refs/heads/}" >&2
    exit 1
  fi
done

exit 0
```

## reference-transaction
Purpose:
- Prevent deleting protected local branches (`git branch -d/-D <branch>`).

Template:

```bash
#!/usr/bin/env bash
set -euo pipefail

state="${1:-}"
if [[ "$state" != "prepared" ]]; then
  exit 0
fi

protected_branches=("main" "master" "staging" "prod")

is_protected_ref() {
  local ref="$1"
  local branch
  for branch in "${protected_branches[@]}"; do
    if [[ "$ref" == "refs/heads/$branch" ]]; then
      return 0
    fi
  done
  return 1
}

zero40="0000000000000000000000000000000000000000"

# Input per line:
# <old-oid> <new-oid> <refname>
while read -r _old_oid new_oid refname; do
  [[ -z "${refname:-}" ]] && continue

  if [[ "$new_oid" == "$zero40" ]] && is_protected_ref "$refname"; then
    echo "reference-transaction: refusing to delete protected local branch: ${refname#refs/heads/}" >&2
    exit 1
  fi
done

exit 0
```

## Install Guidance
Use repo-local versioned hooks when available (recommended):

```bash
./scripts/install-githooks.sh
```

Or install manually into `core.hooksPath`:
- Add executable `pre-push`
- Add executable `reference-transaction`
- Ensure `git config core.hooksPath` points to that directory

Only offer installation when safeguards are absent. Do not overwrite an existing branch-protection system unless requested.

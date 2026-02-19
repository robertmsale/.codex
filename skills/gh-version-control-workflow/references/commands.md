# GH + Git Command Matrix

## Daily Queue
```bash
gh issue list --state open --assignee @me
gh issue list --state open --label ready
gh pr status
```

## Version Bump Tracker
```bash
TARGET_VERSION="<x.y.z>"
TRACKER_MATCHES="$(gh issue list --state open --json number,title --limit 200 --jq ".[] | select(.title == \"Version Bump: ${TARGET_VERSION}\") | .number")"
TRACKER_COUNT="$(printf '%s\n' "$TRACKER_MATCHES" | sed '/^$/d' | wc -l | tr -d ' ')"
[[ "$TRACKER_COUNT" -le 1 ]] || { echo "multiple open trackers for ${TARGET_VERSION}" >&2; exit 1; }
TRACKER_ISSUE="$(printf '%s\n' "$TRACKER_MATCHES" | head -n1)"
if [[ -z "$TRACKER_ISSUE" ]]; then
cat > /tmp/version-bump-issue.md <<'EOF'
## Scope
Release tracker for v<x.y.z>.

## Candidate Issues
- #<issue-number>

## QA Checklist
- <manual testing steps>
EOF
TRACKER_URL="$(gh issue create --title "Version Bump: ${TARGET_VERSION}" --body-file /tmp/version-bump-issue.md --label backlog --assignee "@me")" || { echo "failed to create tracker" >&2; exit 1; }
TRACKER_ISSUE="$(printf '%s\n' "$TRACKER_URL" | awk -F/ 'END{print $NF}')"
[[ -n "$TRACKER_ISSUE" ]] || { echo "failed to parse tracker issue number" >&2; exit 1; }
fi
gh issue comment "$TRACKER_ISSUE" --body "Tracking issue #<issue-number> for v<x.y.z>."
```

## Issue Management
```bash
gh issue create --title "<title>" --body-file /tmp/issue-body.md --label "<label>" --assignee "@me"
gh issue edit <issue-number> --body-file /tmp/issue-body.md
gh issue view <issue-number>
gh issue edit <issue-number> --add-label in-progress --remove-label ready
gh issue comment <issue-number> --body "<update>"
gh issue close <issue-number> --reason completed
```

## Branch + Worktree
```bash
gh issue develop <issue-number> --base <integration-branch> --name "codex/issue-<issue-number>-<slug>"
git worktree add ../<repo>-wt-<issue-number> codex/issue-<issue-number>-<slug>
git worktree list
```

## Development Loop (inside issue worktree)
```bash
# bootstrap PR early
git commit --allow-empty -m "chore: bootstrap PR"
git push -u origin codex/issue-<issue-number>-<slug>

# then iterate normally
git status
git add -A
git commit -m "<type>: <summary>"
git push
```

## Pull Requests
```bash
gh pr create --base <integration-branch> --head codex/issue-<issue-number>-<slug> --title "<title>" --body-file /tmp/pr-body.md
gh pr edit <pr-number> --body-file /tmp/pr-body.md
gh pr view --web
gh pr checks --watch
gh pr merge --squash --delete-branch
for issue in $(gh pr view <pr-number> --json closingIssuesReferences --jq '.closingIssuesReferences[].number'); do gh issue edit "$issue" --add-label qa; done
gh issue comment "$TRACKER_ISSUE" --body-file /tmp/version-bump-qa.md
```

## Sync and Cleanup
```bash
git switch <integration-branch>
git pull --ff-only origin <integration-branch>
git fetch origin --prune
git worktree remove ../<repo>-wt-<issue-number>
git branch -d codex/issue-<issue-number>-<slug> || git branch -D codex/issue-<issue-number>-<slug>
```

## Cleanup after web merge
If PR was merged on GitHub (not via `gh pr merge`), run:
```bash
gh pr view <pr-number> --json state,mergedAt,url
git switch <integration-branch>
git pull --ff-only origin <integration-branch>
git fetch origin --prune
git worktree remove ../<repo>-wt-<issue-number>
git branch -d codex/issue-<issue-number>-<slug> || git branch -D codex/issue-<issue-number>-<slug>
```

## Troubleshooting
If issue/PR body text contains literal `\n`, rewrite using `--body-file`:
```bash
cat > /tmp/pr-body.md <<'EOF'
## Summary
- ...

Closes #<issue-number>
EOF
gh pr edit <pr-number> --body-file /tmp/pr-body.md
```

If git prompts for GitHub credentials despite `gh auth status` success:
```bash
gh auth setup-git
```

If your desired base branch does not exist on remote:
```bash
git push -u origin <integration-branch>
```

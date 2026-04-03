# Recovery Commands

This file is intentionally limited to commands that are useful when the
sanctioned gitops scripts do not cover the situation.

Do not use this file for normal issue, worktree, branch, staging, commit,
sync, merge, or cleanup flows. Use the scripts in
`~/.codex/skills/gh-version-control-workflow/scripts/` for those.

## Repair body formatting

If an issue or PR body contains literal `\n`, rewrite it with `--body-file`:

```bash
cat > /tmp/body.md <<'EOF'
## Summary
- ...

Closes #<issue-number>
EOF

gh issue edit <issue-number> --body-file /tmp/body.md
gh pr edit <pr-number> --body-file /tmp/body.md
```

## Publish a missing remote base branch

If the intended integration branch exists locally but not on `origin`:

```bash
git push -u origin <integration-branch>
```

## Inspect a PR merged outside the script flow

If a PR was merged on GitHub and local state now needs cleanup or verification:

```bash
gh pr view <pr-number> --json state,mergedAt,url,closingIssuesReferences
```

Use that output to confirm the merge before running the sanctioned cleanup
scripts from the main workflow.

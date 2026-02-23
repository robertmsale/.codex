---
name: assign-agent
description: Launch delegated Codex workers in dedicated tmux sessions via codex-tmux, with resumable CODEX_THREAD_ID handoff for GitHub issue workflows. [skill-hash:da921a4]
---

# Assign Agent

Use this skill when an orchestrator agent needs to assign work to another local Codex agent.

## Purpose
- Start a worker in its own tmux session using `codex-tmux`.
- Keep worker sessions resumable by capturing `CODEX_THREAD_ID`.
- Standardize issue delegation prompts and handoff artifacts.

## Runtime-Aware Dispatch Mode (Strict)
- If `ROBDEX_ORCHESTRATION_ENABLED=1`, prefer robdex runtime orchestration over tmux:
  - Use `$robdex-orchestrator` commands (`env-check`, `whoami`, `spawn`, `send`, `wait`, `resume`).
  - Do not launch a tmux worker unless the user explicitly asks for tmux mode.
- If robdex env is unavailable, use tmux mode as the fallback.

## Worker Modes (Strict)
- Choose one mode per assignment and state it explicitly in the prompt.
- `PR worker`:
  - Includes implementation + PR lifecycle.
  - Must follow review loop requirements.
- `Exploratory worker`:
  - Research/triage/verification only.
  - Must not bootstrap PRs or run merge/cleanup steps unless explicitly requested.

## Inter-Agent Message Contract (Strict)
- For orchestrated collaboration, use this prefix:
  - `[<sender-name>] [intent:<plan|ask|handoff|status>] [reply:<required|optional>] <message>`
- Keep messages short and actionable.
- For handoffs, include acceptance criteria and concrete next command(s).
- Never send multiple semantically-duplicate steering messages in rapid succession.

## Abuse Guardrails (Strict)
- Max worker spawns per orchestration turn: 2 (unless user overrides).
- Max delegation depth: 2 (orchestrator -> worker -> optional sub-worker).
- Reuse/`resume` existing workers when possible before spawning new ones.
- If there is no user request for fan-out, do not parallelize aggressively.

## Critical Resume-ID Rule (Strict)
- Resume handoff must use a real, expanded thread id value.
- Workers must run `echo $CODEX_THREAD_ID` and use that output in the final command.
- Never post a literal template such as `$(printenv CODEX_THREAD_ID)` in comments/files.
- Never post an empty thread id (`CODEX_THREAD_ID=''`).
- If `echo $CODEX_THREAD_ID` is empty, stop and resolve before posting handoff instructions.

## PR Review Loop Requirement (Strict)
- If the assignment includes creating or updating a PR, review is mandatory before stopping.
- For PR assignments, bootstrap the PR early with an empty commit (`chore: bootstrap PR`) so review tooling always has an open PR target before the first real implementation commit.
- Worker must request review, apply fixes for findings, and request review again in a loop.
- Worker stops only when review is clean (no unresolved findings) or the orchestrator explicitly says to stop.
- Prefer using the `request-review` skill/script directly:
  - `~/.codex/skills/request-review/scripts/request-review "<commit-message>"`
- `request-review` results are returned in stdout of the worker terminal; do not route review through tmux pane callbacks.

## Required Skill Injection (Strict)
- Orchestrator prompts must explicitly include required skills for the assignment and state `do not skip`.
- Do not rely on implicit skill discovery for delegated workers.
- If running inside robdex orchestration environment, include:
  - `$robdex-orchestrator` (`/Users/robertsale/Code/robdex/skills/robdex-orchestrator/SKILL.md`)
- For any code-change task that ends in a PR, always require:
  - `$gh-version-control-workflow`
- If the task requires branch/worktree cleanup, always require:
  - `$gh-version-control-workflow` (use `git_worktree_cleanup`)
- If the task requires deleting files/directories, always require:
  - `$safe-delete`
- Include absolute SKILL.md paths in the prompt for each required skill.
- If a required skill is unavailable in worker context, worker must state that explicitly and use the closest safe fallback workflow.

## Required launcher
- Launcher path: `/Users/robertsale/.bin/codex-tmux`
- Skill script path: `scripts/codex-tmux` (hard-linked copy of the launcher)

## Session naming
Use:
- `<repo-name>-<issue-number>-<brief-description>`

Examples:
- `coolproject-70-wa-geo-tax-lookup`
- `portal-144-auth-retry`

## Orchestrator launch rule
Always clear `CODEX_THREAD_ID` for new workers:

```bash
SESSION_NAME="<repo>-<issue>-<slug>"
PROMPT="<assignment prompt>"
CODEX_THREAD_ID= codex-tmux "$SESSION_NAME" "$PROMPT"
```

## Assignment prompt template
Seed the worker with enough context to begin immediately:

```text
You are assigned to <issue/reference> in <repo path or repo URL>.

Required skills for this assignment (do not skip):
- $gh-version-control-workflow (<absolute-skill-path>)
- $safe-delete (<absolute-skill-path>) [when deletion applies]

Goals:
1. Research and triage the issue.
2. Capture your real thread id and validate it is non-empty:
   ```bash
   THREAD_ID="$(echo $CODEX_THREAD_ID)"
   test -n "$THREAD_ID"
   ```
3. If [$gh-version-control-workflow](/Users/robertsale/.codex/skills/gh-version-control-workflow/SKILL.md) is in your current skill list/context:
   - Add a GitHub issue comment with a fenced bash block that uses the expanded value:
     ```bash
     CODEX_THREAD_ID='<expanded-thread-id>' codex-tmux '<session-name>'
     ```
4. If that skill is not available in your current context:
   - Write the expanded resume command to `$WORKTREE_ROOT/codex.env`:
     ```bash
     cat > "$WORKTREE_ROOT/codex.env" <<EOF
     CODEX_THREAD_ID='<expanded-thread-id>' codex-tmux '<session-name>'
     EOF
     ```
5. Use the required skills above to create the issue branch/worktree.
6. If your task includes PR work, bootstrap the PR early with an empty commit before implementation commits:
   ```bash
   git commit --allow-empty -m "chore: bootstrap PR"
   git push -u origin HEAD
   # then open/update the PR
   ```
7. Implement the fix and keep the PR updated.
8. If your task includes PR work, run a strict review loop until clean:
   ```bash
   ~/.codex/skills/request-review/scripts/request-review "<commit-message>"
   ```
   - Address findings, commit, and rerun `request-review`.
   - Repeat until review is clean.
9. If PR work is included: merge only after the review loop is clean, then run required cleanup using `git_worktree_cleanup` (and `safe-delete` where applicable).
```

## Orchestrator Acceptance Check
Before considering handoff complete, verify:
- The posted/saved command contains a concrete thread id value.
- The command does not contain `$(printenv CODEX_THREAD_ID)` or similar unevaluated placeholders.
- The thread id value is not empty.
- The assignment prompt explicitly lists required skills with `do not skip`.
- For PR assignments, prompt explicitly requires `gh-version-control-workflow`.
- For cleanup/deletion assignments, prompt explicitly requires `gh-version-control-workflow` (for `git_worktree_cleanup`) and/or `safe-delete`.

## Worker behavior constraints
- One assignment maps to one worker session.
- Do not reuse an unrelated existing tmux session.
- Keep the session name stable for the life of the issue to preserve resume ergonomics.
- In robdex mode, keep one worker thread name per task stream and prefer resume over duplicate names.

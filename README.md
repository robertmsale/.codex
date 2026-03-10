# Codex Home

This repository is the live `~/.codex` home directory for a heavily customized
Codex setup. It is not just a static config bundle. It contains:

- operator-owned Codex configuration
- local skills and workflow wrappers
- local MCP servers
- execution policy rules
- Robdex orchestration/runtime integration (mcp currently unavailable)

## What This Repo Is For

This repo exists to make the local Codex runtime programmable and reviewable.
The main moving parts are:

- `config.toml`: the primary Codex configuration
- `AGENTS.md`: global agent instructions for this repo
- `skills/`: local skills, workflow docs, and script wrappers
- `mcp/`: local MCP server implementations
- `rules/`: command approval / execpolicy rules

The operating model is script-first:

- use skills for workflows instead of ad hoc commands
- use local wrappers for gitops/orchestration whenever possible
- keep operator-controlled config in canonical files
- test workflow changes directly in this repo before relying on them elsewhere
- scripts are rigorously tested, and therefore allowed using the ./rules API

## High-Level Layout

### Core Config

- [`config.toml`](/Users/robertsale/.codex/config.toml)
  The primary Codex runtime config. This controls model defaults, sandbox
  defaults, notifications, MCP server registration, features, and profile
  definitions.

- [`AGENTS.md`](/Users/robertsale/.codex/AGENTS.md)
  The repo-local operating contract for agents working in this environment.
  This is where command execution, skill usage, noisy-command handling, and
  workflow rules are enforced.

- [`configs/`](/Users/robertsale/.codex/configs)
  Additional project-specific config fragments.

### Skills

- [`skills/`](/Users/robertsale/.codex/skills)
  Local skill library. Each skill usually includes:
  - `SKILL.md` instructions
  - `scripts/` wrappers
  - optional `tests/`

Current important skills include:

- `command-execution`
  Required execution model: every command run yields a stable `job_id`, and
  long-running commands must be resumed via `command_execution_wait`. This is a
  shell manipulation technique, replacing zsh with `launch-job` earlier in PATH
  so the job_id is always visible to agents and cannot be circumvented.

- `command-parser`
  Required wrapper for noisy build/test/lint-style commands when parser
  coverage exists. Positive rules allow trivial/additive/reversable operations
  to run via command parser without approval, non-noisy commands are blocked,
  noisy commands are allowed, interactively guiding agents to use it properly.

- `gh-version-control-workflow`
  Script-first gitops workflow for worktree creation, commit, publish, sync,
  and cleanup. Approvals for all additive operations are allowed. Irreversable
  and destructive commands require approval.

- `request-review`
  Review wrapper used before publishing working-code changes. This now supports
  one-shot-safe review behavior and treats the operator-owned canonical `.env`
  as the source of truth for review mode.

- `robdex-orchestrator`
  Local orchestration workflow for worker agents and Robdex messaging. MCP server
  implementation currently gitignored while under development.

- `flutter-commands`
  Project-wide guardrails for Flutter usage, including the ban on
  `flutter analyze`.

- `safe-delete`
  Non-destructive delete workflow. Throws files in /tmp while avoiding collisions,
  allowing recovery, and respecting sandbox.
  

### MCP Servers

- [`mcp/`](/Users/robertsale/.codex/mcp)
  Local MCP implementations used by this runtime. Current notable servers:
  - `command-execution`

These are real local implementations, not placeholders. Changes here can alter
runtime behavior immediately if the calling path imports them directly, or
after restart if a host/runtime process snapshots them.

### Rules

- [`rules/`](/Users/robertsale/.codex/rules)
  Execpolicy / allowlist rules for approved command patterns. These are part of
  the local safety and automation model.

Examples:

- gitops wrapper allowances
- command-parser allowances for approved noisy command families
- branch-protection / workflow-specific policy

Rule edits typically require restart to affect normal approval flow. The usual
pre-restart validation path is `codex execpolicy`.

### Runtime State

- [`robdex/`](/Users/robertsale/.codex/robdex)
  Robdex state, including thread/project metadata used by orchestration.

- [`sessions/`](/Users/robertsale/.codex/sessions)
- [`archived_sessions/`](/Users/robertsale/.codex/archived_sessions)
- [`state/`](/Users/robertsale/.codex/state)
- [`shell_snapshots/`](/Users/robertsale/.codex/shell_snapshots)
- [`history.jsonl`](/Users/robertsale/.codex/history.jsonl)

These are runtime artifacts, not hand-maintained source files.

## Current Workflow Model

### Command Execution

- Treat command execution as job-based.
- Capture `job_id` on every command.
- If the command is still running, wait on the same job rather than rerunning.

### Noisy Commands

- Use `command-parser` for noisy commands when parser coverage exists.
- Do not run formatting tools by default.

### GitOps

- The normal path is worktree-first and script-first.
- Working-code changes should go through review before publish.
- `git-merge-worktree` is the standard merge wrapper after publish:
  - squash merge the PR
  - delete the remote branch
  - sync the parent integration branch
  - remove and prune the local worktree
  - delete the local feature branch
  - leave branch/worktree state intact if the squash merge itself fails
- `git-worktree-cleanup` now also syncs the parent integration branch after a
  successful remote merge:
  - resolve integration branch from the parent repo by default
  - stash dirty parent-repo changes, including untracked files
  - fast-forward from `origin`
  - restore the stash
  - remove the worktree

### Review

- `request-review` is the standard review wrapper.
- In remote mode it is now one-shot-safe:
  - dirty worktree: commit, push, create/use PR, request review
  - clean worktree: review existing `HEAD` instead of refusing
  - explicit existing-commit review is supported without silently pushing newer
    commits
- The canonical operator config file at
  `~/.codex/skills/request-review/.env` is the source of truth for review mode
  and related operator settings.

### Robdex

- Robdex is the native app/runtime wrapper around `codex app-server` in this
  setup.
- This repo contains Robdex-facing orchestration logic.
- Local config/tooling bugs are fixed here first; upstream Robdex app bugs may
  still require Robdex-side changes and a full app restart.

## Important Files To Treat Carefully

- [`config.toml`](/Users/robertsale/.codex/config.toml)
  Operator-owned runtime config.

- [`robdex/robdex.json`](/Users/robertsale/.codex/robdex/robdex.json)
  Live orchestration/runtime state.

- [`rules/`](/Users/robertsale/.codex/rules)
  Approval policy changes can alter what agents can run.

- [`skills/request-review/.env.example`](/Users/robertsale/.codex/skills/request-review/.env.example)
  Example shape only. The real operator-owned `.env` may exist outside git and
  should be treated as authoritative when present.

## How To Validate Changes

Validation depends on what changed.

Typical examples:

- shell wrapper changes:
  - `bash -n path/to/script`
  - run the script-specific shell test, if present

- Python MCP changes:
  - `python3 -m py_compile ...`
  - targeted `unittest` runs

- rule changes:
  - `codex execpolicy check ...`

- workflow changes:
  - exercise the real wrapper path, not just unit tests

This repo is full of workflow code. For many changes, the real validation is
"did the actual wrapper perform the full workflow safely?"

## Short Version

This repo is the live control plane for a customized Codex + Robdex
environment. The important source files are under `skills/`, `mcp/`, `rules/`,
`config.toml`, and `AGENTS.md`. The rest includes a mix of supporting artifacts. Treat it like operational infrastructure, not a toy
dotfiles repo.

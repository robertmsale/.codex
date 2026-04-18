# codex-privileged-exec-http

## Purpose

This service provides a single unsandboxed command broker for commands that are
blocked or degraded by Codex sandboxing. It is intentionally narrower than a
remote shell:

- policy matching is delegated to vendored `codex-execpolicy`
- shell-shape classification is delegated to vendored `codex-shell-command`
- actual execution is argv-based only
- shell-form commands only qualify when they can be reduced to a single plain
  command and pass an extra strict token check

## Design

### Policy

- Rules are loaded from one or more execpolicy files or watched directories.
- Matching uses `codex-execpolicy` directly, not the CLI JSON wrapper.
- The service hot-reloads via filesystem watching and explicit
  `POST /policy/reload`.
- When a configured input is a directory, the service loads matching
  `*.rules` / `*.codexpolicy` files in lexical order and watches the directory
  for new, removed, and changed files.
- Effective decision mapping:
  - `allow`: command may execute
  - `prompt`: rejected by the service
  - `forbidden`: rejected by the service
  - no match: service declines; caller can fall back to local sandboxed exec

### Command Normalization

- Direct argv requests are eligible as-is.
- Shell-form requests (`bash -lc`, `zsh -lc`, `sh -lc`) are inspected with
  vendored tree-sitter helpers.
- Only a single plain command is eligible for privileged execution.
- Multi-command shell sequences, redirections, substitutions, control flow, and
  other complex shell constructs are rejected from the privileged path.

### Execution

- The service executes with `tokio::process::Command`.
- `cwd` is explicit and must exist.
- `envOverrides` are supported with identifier validation.
- Output is capped.
- Command runtime is not time-limited by this service.

## Endpoints

- `GET /healthz`
- `POST /policy/check`
- `POST /policy/reload`
- `POST /exec/run`

## Safety Constraints

- No arbitrary shell string execution.
- No implicit `bash -lc`.
- No policyless privileged execution.
- No `prompt` passthrough for now.
- No async job system in v1.

## Current Status

- [x] Workspace crate scaffold
- [x] Vendored `execpolicy` integration
- [x] Vendored shell classification integration
- [x] Strict shell-token gate
- [x] Hot-reload by mtime + explicit reload endpoint
- [x] Filesystem watcher reload + directory discovery
- [x] Synchronous run endpoint with output caps
- [x] Unit tests for normalization and policy behavior
- [ ] Async job execution
- [ ] Supervisor runner/template
- [ ] Bridge integration / client routing
- [ ] Audit log persistence
- [ ] Per-rule cwd/env constraints

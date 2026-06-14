# Agent Runtime Roadmap

This roadmap is a planner-facing source of truth for the experimental agent
runtime in `backend/experiments/agent-runtime`. It is intentionally lightweight:
use it to avoid rediscovering settled decisions after compaction, not as a full
product specification.

## Status

- **Current milestone:** Build an alternative, PostgreSQL-backed agent runtime
  that can eventually power a macOS/iOS GUI without depending on stable Robdex
  bridge internals.
- **Current active slice:** GUI contract + proof.
- **Current implementation owner:** Codex Config Operator.
- **Planner stance:** contract-first, backend/Rust proof first, no Flutter UI
  implementation until the GUI state/operation boundary is stable.

## Owner principles

- PostgreSQL is the canonical runtime source of truth.
- JSON/Markdown files are seed/import/export artifacts unless explicitly stated
  otherwise.
- No legacy tombstones, fallback tooling, or deprecated duplicate paths.
- Tool/function schemas must remain cache-stable; dynamic context belongs later
  in request input or runtime-managed context, not schema text.
- Roles are DB-backed, hot-updatable runtime policy/config records.
- Command registry entries are live DB data, not const-compiled app behavior.
- Workers request command-registry changes; approvers choose final scope and
  policy.
- Durable state belongs in Rust/Postgres. Dart may own only widget-local
  ephemeral state.
- Requirements should be task-specific. Project-wide invariants belong in
  composables or role/project docs.
- Validation scripts must use isolated validation databases and must not mutate
  the normal runtime database by default.

## Source of truth map

- Workspace root: `backend/experiments/agent-runtime/`
- Shared projection/reducer types:
  `crates/robdex-agent-runtime-projection/src/lib.rs`
- Runtime DB/schema/helpers: `crates/robdex-agent-runtime/src/db.rs`
- Server/API/WebSocket routes: `crates/robdex-agent-runtime/src/server.rs`
- Snapshot/delta adapters: `crates/robdex-agent-runtime/src/projection.rs`
- Runtime send loop/model boundary: `crates/robdex-agent-runtime/src/runtime.rs`
- Model adapter: `crates/robdex-agent-runtime/src/model/`
- Starlark execution host: `crates/robdex-agent-runtime/src/starlark_host/`
- Command registry: `crates/robdex-agent-runtime/src/command_registry.rs`
- Workflow memory: `crates/robdex-agent-runtime/src/workflow_memory.rs`
- Resident operations/startup/shutdown: `crates/robdex-agent-runtime/src/operations.rs`
- GUI sync client prototype: `crates/robdex-agent-runtime/src/gui_sync.rs`
- Local service scripts: `scripts/agent-runtime-service.sh` and
  `scripts/validate-local-service.sh`
- README/user docs: `README.md`

## Completed slices

1. Nested experimental runtime workspace isolated from stable Robdex.
2. Host PostgreSQL-backed state and schema initialization.
3. Starlark `execute_code` runtime surface.
4. Kernel hardening around filesystem, git protection, mutation actions, and
   execution roots.
5. DB-backed role policy system with import/export/archive/version behavior.
6. Approvals and role routing.
7. Action-only approval resume.
8. Test hygiene and isolated validation DB helpers.
9. DB-backed typed command registry.
10. Agent-facing scoped command-registry requests.
11. Session-only async process manager.
12. Persistent session lifecycle: metadata, archive, close, fork, history.
13. Approver ergonomics for command-registry review/preview/decision flow.
14. Role admin backend routes/CLI.
15. Agent-led workflow memory using pgvector/LM Studio embeddings.
16. Projection-first snapshot/reducer foundation.
17. Resident server MVP.
18. Semantic WebSocket deltas.
19. Core admin APIs.
20. Typed API error shape and error mapping polish.
21. Resident server smoke harness.
22. Runtime operations hardening and local service wrapper.
23. GUI integration foundation via shared projection and sync client prototype.
24. Cache-stable command discovery with synthetic runtime command context and
    Starlark `cmd.describe()` affordances.
25. Validation-script cutover for explicit CLI binary selection and sanitized
    model request evidence.

## Active slice: GUI contract + proof

Goal: define the backend/Rust contract that a future Rust/Rinf GUI controller
and Flutter client will consume.

Scope:

- Extend or clarify shared GUI contract types.
- Keep `RuntimeProjection` as the full hydrated source-of-truth UI state.
- Keep `RuntimeDelta` as the only realtime persisted-state update format.
- Model local-only GUI/controller state separately from `RuntimeProjection`.
- Define typed GUI operations/intents and typed outcomes/errors for the first GUI
  integration boundary.
- Document Dart responsibilities and forbidden runtime decisions.
- Add proof tests for serialization, reducer behavior, selected-session switch
  semantics, operation errors, resync surfacing, and snapshot/WebSocket
  convergence.

Non-goals:

- No Flutter UI implementation.
- No stable Robdex production-path changes.
- No fallback state path parallel to PostgreSQL + projection/deltas.

## Validation baseline

Run from `backend/experiments/agent-runtime` unless noted otherwise:

```bash
cargo check
cargo test
scripts/smoke-resident-server.sh
scripts/validate-local-service.sh
```

Run from repo root for whitespace validation:

```bash
git diff --check -- backend/experiments/agent-runtime
```

Common targeted scripts:

```bash
scripts/validate-command-registry.sh
scripts/validate-scoped-command-requests.sh
scripts/validate-session-lifecycle.sh
scripts/validate-db-canonical-roles.sh
scripts/validate-process-manager.sh
scripts/validate-action-resume.sh
scripts/validate-approvals-routing.sh
scripts/validate-approver-ergonomics.sh
scripts/validate-role-admin-ux.sh
scripts/validate-mutation-actions.sh
scripts/validate-workflow-memory.sh
```

Live-model or external-service tests are opt-in only:

```bash
scripts/smoke-live-server-gpt54mini.sh
scripts/smoke-lmstudio-embeddings.sh
```

## Deferred / next likely slices

1. Finish GUI contract + proof.
2. Fill API gaps discovered by the GUI contract.
3. Service packaging beyond local scripts.
4. Rust/Rinf GUI backend integration.
5. Flutter GUI implementation using design-system-only widgets.
6. Broader execution expansion only after GUI/runtime lifecycle is stable.

## Current known risks

- The cumulative experimental runtime diff is large; review by slice and rely on
  validation evidence.
- Projection types can become too raw if GUI-ready summaries are not added where
  needed.
- Operation contracts must not leak durable decision-making into Dart.
- Command/runtime context must remain cache-stable at the model schema layer.
- Validation scripts must track intentionally removed fields such as full
  `requestShape` persistence.

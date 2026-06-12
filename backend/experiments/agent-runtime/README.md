# Experimental Agent Runtime

This nested workspace is isolated from stable Robdex. It is not a member of the
main backend workspace and is not wired into supervisor, the stable bridge, the
stable database, or the GUI.

## Database

The runtime requires host PostgreSQL. Configure the connection with:

```sh
export ROBDEX_AGENT_RUNTIME_DATABASE_URL='postgres://postgres:postgres@127.0.0.1:5432/robdex_agent_runtime'
```

Initialize the schema:

```sh
robdex-agent-runtime init-db
```

Use local host Postgres for all experimental runtime state.

## Model adapter boundary

Runtime code depends on the local `ModelClient` trait. The current adapter is
`model::codex_adapter::CodexBackedModelClient`, an experimental direct
Responses adapter that performs HTTPS calls to the ChatGPT Codex Responses
endpoint with Codex auth material from the local environment or `auth.json`.
It does not claim to use the full vendored Codex provider/client runtime. All
direct HTTP, Responses request shaping, Codex auth headers, SSE parsing, and raw
model response handling must stay inside `model::codex_adapter`.

## Script output

Starlark scripts emit final tool output only through:

```python
output(value)
```

Host API calls return values to the script, but they do not implicitly append to
the final tool output. This keeps tool result packets deterministic and concise.

## Role policy foundation

Postgres is the runtime source of truth for roles. JSON manifests and prompt files in `roles/` are seed/import/export artifacts only. Import resolves prompt files into immutable `role_versions.instruction_text`; runtime session creation reads the current DB role version and stores a complete immutable `sessions.role_snapshot`.

Active actions implemented by this slice:
- `tool.execute_code`
- `fs.read`
- `cmd.rg.run`

Reserved future action names documented but not implemented here:
- `agent.spawn.<role>`
- `agent.archive`
- `requirements.set.self`
- `requirements.set.other`
- `requirements.change.active`
- `message.send`
- `message.route`

Manifest decision values are `allow`, `deny`, `ownerApproval`, and `orchestratorApproval`. Runtime policy maps approval decisions to `approvalRequired` and does not execute those actions in this task. Missing action policy defaults to deny. Policy is execution authority; `capabilities` are validated to exactly match policy keys so they cannot contradict enforcement. Sessions store immutable role snapshots at creation time; turns use the stored snapshot rather than rereading the latest manifest. The direct Responses adapter receives the model name and instruction text from the session snapshot. Reasoning effort is stored in the DB role version and snapshot but is not applied by the current direct adapter yet.

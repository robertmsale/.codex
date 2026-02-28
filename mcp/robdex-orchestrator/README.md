# robdex-orchestrator-mcp

MCP server for robdex bridge orchestration.

## Scope

Tools exposed:

- `robdex_list_projects`
- `robdex_list_agents`
- `robdex_spawn_agent`
- `robdex_unarchive_agent`
- `robdex_rename_agent`
- `robdex_send_message`

Notes:

- `robdex_unarchive_agent` is the tool for restoring archived threads.

Not exposed:

- `whoami`
- `env-check`
- `wait`
- `review-*`

## Policy

- Environment/identity checks are handled transparently by the server.
- `ROBDEX_ORCHESTRATION_ENABLED` is not required for tool usage.
- Bridge defaults are used when unset:
  - host: `127.0.0.1`
  - port: `42080`
  - token: optional (Authorization header sent only when provided)
- Cross-project messaging is restricted to orchestrator threads.
- Tool outputs are compact plain text.

## Run

```bash
uv --project ~/.codex/mcp/robdex-orchestrator run robdex-orchestrator-mcp
```

# command-parser-mcp

Sandbox-aware command execution + output summarization MCP server.

## Run

```bash
uv --project ~/.codex/mcp/command-parser run command-parser-mcp
```

## Tool

- `command_parser_run`
  - Executes a command once under sandbox routing derived from current policy.
  - Does not accept sandbox override inputs from callers.
  - Does not accept caller profile overrides; parser profile comes from operator config.
  - Detects sandbox failures and returns them directly (skips parser analysis).
  - Parses non-sandbox failures/output with `codex exec` extraction profile.
  - Returns plain text only:
    - sandbox failure text (if blocked), or
    - parser extraction output (on success).
  - Policy precedence:
    1. per-thread metadata in `~/.codex/robdex/robdex.json`
       (`threadMetadataByID[threadId]`)
    2. `~/.codex/config.toml` defaults (`sandbox_mode`, `network_access`)
    3. process env defaults (`ROBDEX_SANDBOX_MODE`, `ROBDEX_NETWORK_ACCESS`)

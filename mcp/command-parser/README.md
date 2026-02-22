# command-parser-mcp

Sandbox-aware command execution + output summarization MCP server.

## Run

```bash
uv --project ~/.codex/mcp/command-parser run command-parser-mcp
```

## Tool

- `command_parser_run`
  - Executes a command once under sandbox routing derived from current policy.
  - Detects sandbox failures and returns them directly (skips parser analysis).
  - Parses non-sandbox failures/output with `codex exec` extraction profile.
  - Returns plain text only:
    - sandbox failure text (if blocked), or
    - parser extraction output (on success).
  - Policy precedence:
    1. explicit tool args (`sandbox_mode`, `network_access`)
    2. per-thread metadata in `~/.codex/robdex.json` (`threadMetadataByID[threadId]`)
    3. `~/.codex/config.toml` defaults (`sandbox_mode`, `network_access`)
    4. process env defaults (`ROBDEX_SANDBOX_MODE`, `ROBDEX_NETWORK_ACCESS`)

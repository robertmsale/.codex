# safe-delete-mcp

MCP server for recoverable file/directory deletion by staging paths under `/tmp/safe-delete`.

## Tool

- `safe_delete(paths, cwd=None)`
  - Resolves sandbox state from runtime metadata/config (no caller overrides).
  - Moves each path to `/tmp/safe-delete/<name>-<timestamp>` (auto-suffixed on collisions).
  - Uses the same sandbox routing style as command-parser; blocked writes return sandbox errors.
  - Returns staged source -> destination mappings.

## Run

```bash
uv --project ~/.codex/mcp/safe-delete run safe-delete-mcp
```

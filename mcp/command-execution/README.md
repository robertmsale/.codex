# command-execution-mcp

Stateless waiter MCP for marker-file jobs launched by:

- `~/.codex/skills/command-execution/scripts/launch-job`

`launch-job` output contract:

- `stderr`: `job_id: <uuid>`
- `stdout`: command output only

State marker location:

- `/tmp/codex-command-jobs/<job_id>.job`

Tool:

- `command_execution_wait(job_id)`

Run:

```bash
uv --project ~/.codex/mcp/command-execution run command-execution-mcp
```

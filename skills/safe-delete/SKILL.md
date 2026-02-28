---
name: safe-delete
description: Use MCP `safeDelete.safe_delete` to stage deletions under `/tmp/safe-delete` with collision-safe timestamped paths. Prefer this skill whenever a task involves removing files or directories. [skill-hash:9e1d4bf]
---

# Safe Delete

Use this skill whenever a task involves deleting files or directories.

Rules:

- Never use `rm`.
- Use MCP `safeDelete.safe_delete` so paths are moved to `/tmp/safe-delete` and
  can be recovered.
- If MCP is unavailable, fallback to:
  - `mkdir -p /tmp/safe-delete`
  - `mv -- <path...> /tmp/safe-delete/`

Patterns:

- Single path: `safeDelete.safe_delete(paths=[\"<path>\"])`
- Multiple paths: `safeDelete.safe_delete(paths=[\"<path1>\", \"<path2>\"])`
- Optional cwd: `safeDelete.safe_delete(paths=[\"build\"], cwd=\"/repo\")`

Quick check:

- `ls -la /tmp/safe-delete`

---
name: safe-delete
description: Use the `trash` CLI for deletions so files are recoverable. Prefer this skill whenever a task involves removing files or directories.
---

# Safe Delete

Use this skill whenever a task involves deleting files or directories.

Rules:
- Never use `rm`.
- Use `trash` for deletions so items go to the OS Trash and can be recovered.
- For paths that might start with `-`, pass `--` before paths.

Patterns:
- Single path: `trash -- <path>`
- Multiple paths: `trash -- <path1> <path2>`
- Glob delete: `trash '*.log'`

Quick check:
- `which trash`

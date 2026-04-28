---
name: privileged-exec
description: Ran into a sandbox issue, approval request, or privileged-exec rejection? Use this skill immediately for the sanctioned resolution path. [skill-hash:ace65f3]
---

# Privileged Exec

Use this skill when a command hits sandbox friction, triggers an approval request, or is rejected by privileged execution.

## Resolution Path

- Re-run the needed command through the sanctioned public tool surface for the task.
- Run commands plainly, synchronously, and sequentially.
- Use one command at a time and wait for it to finish before issuing the next.
- Simple `&&` and `||` chains are supported only when every segment is a plain sanctioned command.

## Command Shape Rules

- Use `&&` or `||` only for simple chains of plain commands where every command is independently sanctioned.
- Do not use command separators like `;`.
- Do not use pipes.
- Do not use command substitution.
- Do not use subshells.
- Do not use shell expansions or wrappers that change the command shape.
- Do not prepend inline env assignments.

## What Counts As Sanctioned

- Shared skill scripts under `~/.codex/skills/*/scripts/*` are the default sanctioned privileged entrypoints when the active skill tells you to use them. Your CWD may include `<CWD>/.codex/skills/*/scripts/*` which are (or should be) added to privileged execution.
- Some non-skill tools are sanctioned by the active workflow. If the current skill or role explicitly tells you to use a tool, follow that instruction plainly.
- `public-dev-tunnel` is the sanctioned public HTTPS tunnel wrapper for local dev callbacks. It uses `cloudflared` and prints only the public base URL on `start`, `url`, and running `status`.

## Public Dev Tunnels

- Start: `public-dev-tunnel start http://127.0.0.1:<port> <name>`
- Read URL: `public-dev-tunnel url <name>`
- Status: `public-dev-tunnel status <name>`
- Logs: `public-dev-tunnel logs <name>`
- Stop: `public-dev-tunnel stop <name>`
- For Ezra QBO dev router OAuth, set `QBO_OAUTH_CALLBACK_DEV_FORWARD_BASE_URL` to the URL printed by `public-dev-tunnel`.
- If `cloudflared` is missing, stop and report that setup is required: `brew install cloudflared`.

## What Not To Assume

- Do not assume an arbitrary shell command should be approved just because it seems necessary.
- Do not assume repo-local scripts, ad hoc wrappers, or rewritten command variants are privileged.
- If the active skill gives you a sanctioned script, use that script instead of reconstructing the workflow manually.

## Failure Handling

- If a sanctioned, non-destructive, necessary command still triggers an approval request or privileged-exec rejection, report the exact command and the relevant error output to the user or orchestrator.
- Treat that as a tooling failure.
- Do not route around it by approving the command, splitting into unsafe variants, or inventing a new wrapper.
- If there is a necessary command that requires ENV_VAR= prefixes in order to work properly, and there is absolutely no available and sanctioned alternative to it, this must be reported so a sanctioned script with suffixed args can be considered for addition to the privileged execution system.
- `Codex Config Operator` is the owner of the privileged execution pipeline.

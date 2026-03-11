---
name: xcode-mcp
description: Xcode MCP is disabled on this system. Use direct `xcodebuild` with the approval system instead of relying on the Xcode MCP server. Open a dedicated Xcode window first for worktrees when you need GUI inspection. [skill-hash:6e2c4b9]
---

# Xcode MCP

Use this skill for Apple-platform projects that live in Xcode.

Default rule:
- the Xcode MCP server is disabled here
- use direct `xcodebuild` and rely on approval prompts when escalation is needed
- do not route `xcodebuild` through the Xcode MCP server on this system

Practical consequence:
- `xcodebuild` is the one exception to the normal noisy-command routing rule
- direct `xcodebuild` is acceptable
- `command-parser xcodebuild ...` is also acceptable when you specifically want output extraction
- both paths should prompt for approval rather than being auto-allowed

## Setup

If you are working inside a linked git worktree:
- open the `.xcodeproj` or `.xcworkspace` from that worktree first
- use `open <path-to-project>` or `open <path-to-workspace>` so the worktree gets its own Xcode window
- do not reuse a base-repo Xcode window for a worktree task

After opening:
- use `XcodeListWindows` to locate the correct workspace tab identifier
- use that tab identifier for all later Xcode MCP calls

## Device Guardrails

Do not build or run on a physical device unless its visible name is exactly one of:
- `Rob`
- `My Mac`
- a simulator

If the target device has any other human name:
- treat it as a coworker device
- do not try to build, run, install, or test on it
- switch to a simulator or `My Mac`

## Preferred Workflow

1. Open the worktree-local `.xcodeproj` or `.xcworkspace` if needed.
2. Use Xcode itself for GUI inspection if needed.
3. Make edits through the normal file-editing tools.
4. Validate with direct `xcodebuild` commands and the approval system.

## Guardrails

- Do not assume the Xcode MCP server is available.
- Do not use a shared Xcode window for multiple worktrees.
- Do not build against random named devices.
- Prefer targeted `xcodebuild test` invocations before broad test runs when the affected area is narrow.

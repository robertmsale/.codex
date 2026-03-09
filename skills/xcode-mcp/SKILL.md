---
name: xcode-mcp
description: Use the Xcode MCP tools instead of `xcodebuild` or ad-hoc Apple CLI flows when working on Xcode projects. Open a dedicated Xcode window first for worktrees, then inspect files, builds, tests, previews, and docs through the MCP surface. [skill-hash:7b3e1c4]
---

# Xcode MCP

Use this skill for Apple-platform projects that live in Xcode.

Default rule:
- prefer the Xcode MCP server over `xcodebuild`, `swift build`, ad-hoc DerivedData inspection, or manual Xcode GUI guesswork

This is the main MCP surface visible here right now:
- `XcodeListWindows`
  - find the active Xcode windows and their workspace tab identifiers
- `XcodeLS`
  - browse the project navigator structure
- `XcodeGlob`
  - find files by wildcard
- `XcodeGrep`
  - search project text with regex
- `XcodeRead`
  - read source files with line numbers
- `XcodeUpdate`
  - edit existing files
- `XcodeWrite`
  - create or overwrite files
- `XcodeRefreshCodeIssuesInFile`
  - refresh file-level diagnostics
- `XcodeListNavigatorIssues`
  - inspect the Issue Navigator view
- `DocumentationSearch`
  - search Apple documentation from inside the Xcode toolchain context
- `BuildProject`
  - build the active scheme for the selected workspace tab
- `GetBuildLog`
  - inspect the last build log and filter errors/warnings
- `GetTestList`
  - enumerate available tests
- `RunAllTests`
  - run the active test plan
- `RunSomeTests`
  - run specific tests only
- `RenderPreview`
  - render SwiftUI previews
- `ExecuteSnippet`
  - run a Swift snippet in file context

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
2. Use `XcodeListWindows` to find the right tab.
3. Inspect project structure with `XcodeLS`, `XcodeGlob`, `XcodeGrep`, and `XcodeRead`.
4. Make edits through the normal file-editing tools.
5. Validate with `BuildProject`, `GetBuildLog`, `RunSomeTests`, `RunAllTests`, `RenderPreview`, or `XcodeRefreshCodeIssuesInFile` as appropriate.
6. Use `DocumentationSearch` when Apple API behavior is unclear.

## Guardrails

- Do not default to CLI builds when the Xcode MCP tool can do the same job.
- Do not guess the active workspace tab; check it with `XcodeListWindows`.
- Do not use a shared Xcode window for multiple worktrees.
- Do not build against random named devices.
- Prefer targeted tests with `RunSomeTests` before broad test runs when the affected area is narrow.

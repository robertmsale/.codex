# Implementation Plan Seed

This is not the full migration plan yet. It is the development scaffold for
planning and implementation review.

## Shared foundations

1. Build a small shared runtime layer in `codex-backend-core`.
2. Standardize:
   - bind/port parsing
   - tracing setup
   - health response model
   - graceful shutdown
   - subprocess execution helpers
3. Add fixture-based parity tests around the current Python/Deno behavior before
   swapping any live supervisor program.

## Service tracks

### codex-aux-http

- document current HTTP routes and command execution contract
- implement minimal Rust clone
- compare responses against the Deno server

### codex-gitops-http

- separate bridge/path-translation logic from git operation dispatch
- encode operation schemas instead of untyped dict payloads where possible
- keep compatibility surface for existing workflow scripts during transition

### flutter services

- isolate simulator/device/process management into library modules
- model artifacts/log directories explicitly
- preserve current env vars and output locations

### supervisor

- move canonical supervisor templates under this workspace
- eventually add generation/validation tooling rather than hand-editing `.ini`
  files in another repo


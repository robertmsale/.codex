# Command Parser Decommission

The command-parser workflow is no longer part of the desired active Codex
operation model. It previously reduced noisy shell output, but after quota
accounting changed it now adds unnecessary overhead.

## Inventory

- Role instructions: `roles/command-parser.md`
- Skill docs and wrappers: `skills/command-parser/`
- Root scripts: `scripts/command-parser`, `scripts/zsh-command-parser`,
  `scripts/command-parser.env`, `scripts/command-parser.rule`,
  `scripts/output_postprocess.sh`
- Shell execution coupling: `scripts/zsh` sourced `output_postprocess.sh` and
  routed large command output through `/v1/command-parser/parse`
- Config profiles: `[profiles.command-parser-spark]` and
  `[profiles.command-parser]` in `config.toml`
- Backend Rust crate: `backend/crates/codex-command-parser-client` called the
  parser route
- Backend Python/Deno-era service code:
  `backend/python/codex-services/src/codex_aux_http.ts`
- Supervisor/support service: no active support service remains for the parser
  route
- Rules/policies: `robdex-rules/12-skill-scripts.codexpolicy` allowed parser
  skill wrappers; `robdex-rules/30-noisy-command-parser.codexpolicy` duplicated
  noisy-command policy naming
- Tests: `skills/command-parser/tests/*`
- Docs: `README.md`, `backend/README.md`, and backend migration notes referenced
  command-parser as active or current

## Decommission Slices

1. Docs and ownership map: add this inventory and
   `docs/instruction-ownership.md`.
2. Backend decoupling: remove command-parser routes and client workspace
   membership.
3. Shell wrapper decoupling: remove automatic command-parser postprocessing from
   normal shell execution while preserving PATH, privileged-exec, live-process
   registration, synchronous execution, and exit-code behavior.
4. Script/skill/rule cleanup: remove active command-parser wrappers, role,
   skill, and privileged-exec allowances after backend and shell decoupling.
5. Config cleanup: remove command-parser profiles once no active path needs
   them.
6. Validation/restart: rebuild and restart only affected support services, then
   verify shell, privileged-exec, Robdex CLI, and Requirements behavior.

## Shell Recovery Bypass

`scripts/zsh` retains an opt-in recovery bypass:
`CODEX_ZSH_PLAIN_EXEC=1`. It is positioned after PATH reconstruction,
activation, cwd normalization, and command parsing, and before
privileged-exec/live-process/output-capture handling. It is kept as an
operator recovery switch for future wrapper maintenance; normal execution does
not set it.

## Retained Compatibility

No compatibility shim keeps command-parser active by default. If a future manual
parser use case is needed, it should be reintroduced as a new explicit tool with
a fresh cost/benefit review.

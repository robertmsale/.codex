# Robdex Bridge Rust Port

Initial Rust port goals:
- keep the bridge library-first and unit testable
- isolate storage tests to temp directories only
- split transport, store, config, and transform logic into narrow modules
- preserve the existing HTTP surface before replacing the live Deno bridge

Current crate:
- `crates/codex-robdex-bridge`
- `crates/codex-app-server-adapter`

Pinned upstream source:
- git submodule at `backend/vendor/codex`
- pinned commit: `e9fb49366c93a1478ec71cc41ecee415a197d036`
- intended version label: `rust-v0.124.0`

Current implemented slices:
- tmp-safe SQLite state store using `sqlx`
- pure transform helpers for role instruction loading, delta merge, cache pruning, and agent summary shaping
- thin HTTP routes for `GET /healthz`, `GET /info`, `GET /state/snapshot`, `GET /threads/messages`, and `GET /events/replay`
- placeholder `GET /ws` returning `501` until websocket fanout is ported

Explicitly deferred:
- app-server websocket attachment
- bridge websocket fanout to clients
- command mutation/query websocket commands
- deeper reuse of Codex internal runtime/state logic beyond the initial adapter crate

Codex source reuse plan:
- use the pinned submodule as the source of truth for protocol/runtime reuse
- expose only the upstream crates we actually need through `codex-app-server-adapter`
- do not couple the bridge binary directly to raw upstream crates outside that adapter seam

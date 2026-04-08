# Robdex Frontend

Flutter client scaffold for the next Robdex iteration.

## Shape

- `robdex_app/`: Flutter shell
- `backend/crates/robdex-protocol`: shared DTOs
- `backend/crates/robdex-client-core`: Rust bridge client/runtime core
- `backend/crates/robdex-rinf-bridge`: Flutter-facing Rust bridge entry point

## Intent

The Swift client remains the live operational UI for now. This Flutter app is the new composable frontend surface where protocol, session behavior, and DTOs can evolve together with the Rust bridge instead of drifting apart.

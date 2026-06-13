#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
source scripts/lib-validation-db.sh
validation_setup_database

export ROBDEX_AGENT_RUNTIME_EMBEDDING_PROVIDER=deterministic
export ROBDEX_AGENT_RUNTIME_EMBEDDING_DIMENSIONS=2560

if ! psql "$ROBDEX_AGENT_RUNTIME_DATABASE_URL" -At -c "select 1 from pg_available_extensions where name='vector'" | rg -q '^1$'; then
  printf 'pgvector extension package is not available in this Postgres installation; install pgvector before running workflow-memory validation.\n' >&2
  exit 2
fi

CARGO_BIN="${ROBDEX_AGENT_RUNTIME_CARGO:-$(rustup which cargo)}"
printf '\n$ %s test --package robdex-agent-runtime --lib workflow_memory_deterministic_validation -- --ignored\n' "$CARGO_BIN"
"$CARGO_BIN" test --package robdex-agent-runtime --lib workflow_memory_deterministic_validation -- --ignored

printf '\n[workflow memory validation complete]\n'

#!/usr/bin/env bash

VALIDATION_DB_PREFIX="robdex_agent_runtime_validation_"
DEFAULT_RUNTIME_DATABASE_URL="postgres://postgres:postgres@127.0.0.1:5432/robdex_agent_runtime"
DEFAULT_VALIDATION_ADMIN_DATABASE_URL="postgres://postgres:postgres@127.0.0.1:5432/postgres"

validation_db_name_from_url() {
  local url="$1"
  printf '%s' "${url##*/}" | sed 's/[?].*$//'
}

validation_admin_url_without_db() {
  local url="$1"
  local before_db="${url%/*}"
  printf '%s' "$before_db"
}

validation_require_prefixed_db_name() {
  local db_name="$1"
  case "$db_name" in
    ${VALIDATION_DB_PREFIX}*) return 0 ;;
    *)
      printf '[validation-db] refusing destructive cleanup for non-validation database: %s\n' "$db_name" >&2
      printf '[validation-db] required prefix: %s\n' "$VALIDATION_DB_PREFIX" >&2
      return 1
      ;;
  esac
}

validation_create_database() {
  VALIDATION_ADMIN_DATABASE_URL="${ROBDEX_AGENT_RUNTIME_VALIDATION_ADMIN_DATABASE_URL:-$DEFAULT_VALIDATION_ADMIN_DATABASE_URL}"
  NORMAL_RUNTIME_DATABASE_URL="${ROBDEX_AGENT_RUNTIME_DATABASE_URL:-$DEFAULT_RUNTIME_DATABASE_URL}"
  NORMAL_RUNTIME_DB_NAME="$(validation_db_name_from_url "$NORMAL_RUNTIME_DATABASE_URL")"
  VALIDATION_DB_NAME="${ROBDEX_AGENT_RUNTIME_VALIDATION_DATABASE_NAME:-${VALIDATION_DB_PREFIX}$(date +%Y%m%d%H%M%S)_$$_${RANDOM}}"
  validation_require_prefixed_db_name "$VALIDATION_DB_NAME"
  if [[ "$VALIDATION_DB_NAME" == "$NORMAL_RUNTIME_DB_NAME" ]]; then
    printf '[validation-db] refusing to use normal runtime database as validation database: %s\n' "$VALIDATION_DB_NAME" >&2
    return 1
  fi
  VALIDATION_DATABASE_URL="$(validation_admin_url_without_db "$VALIDATION_ADMIN_DATABASE_URL")/$VALIDATION_DB_NAME"
  export ROBDEX_AGENT_RUNTIME_DATABASE_URL="$VALIDATION_DATABASE_URL"
  printf '[validation-db] creating isolated database: %s\n' "$VALIDATION_DB_NAME"
  printf '[validation-db] admin connection: %s\n' "$VALIDATION_ADMIN_DATABASE_URL"
  printf '[validation-db] runtime connection: %s\n' "$VALIDATION_DATABASE_URL"
  psql "$VALIDATION_ADMIN_DATABASE_URL" -v ON_ERROR_STOP=1 -c "CREATE DATABASE \"$VALIDATION_DB_NAME\"" >/dev/null
}

validation_cleanup_database() {
  local status=$?
  if [[ -z "${VALIDATION_DB_NAME:-}" || -z "${VALIDATION_ADMIN_DATABASE_URL:-}" ]]; then
    return "$status"
  fi
  if ! validation_require_prefixed_db_name "$VALIDATION_DB_NAME"; then
    printf '[validation-db] cleanup refused. Leftover database was not touched.\n' >&2
    return "$status"
  fi
  printf '[validation-db] cleaning isolated database: %s\n' "$VALIDATION_DB_NAME"
  if ! psql "$VALIDATION_ADMIN_DATABASE_URL" -v ON_ERROR_STOP=1 -c "DROP DATABASE IF EXISTS \"$VALIDATION_DB_NAME\" WITH (FORCE)" >/dev/null; then
    printf '[validation-db] cleanup failed. Leftover database: %s\n' "$VALIDATION_DB_NAME" >&2
    printf '[validation-db] manual cleanup connection: %s\n' "$VALIDATION_ADMIN_DATABASE_URL" >&2
    printf '[validation-db] manual cleanup SQL: DROP DATABASE IF EXISTS \"%s\" WITH (FORCE);\n' "$VALIDATION_DB_NAME" >&2
  fi
  return "$status"
}

validation_setup_database() {
  validation_create_database
  trap validation_cleanup_database EXIT
  if [[ "${ROBDEX_AGENT_RUNTIME_VALIDATION_FORCE_FAIL_AFTER_SETUP:-}" == "1" ]]; then
    printf '[validation-db] forced failure after setup requested\n' >&2
    return 42
  fi
}

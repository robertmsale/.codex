CREATE TABLE IF NOT EXISTS sessions (
    id UUID PRIMARY KEY,
    status TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb
);

CREATE TABLE IF NOT EXISTS event_stream (
    sequence BIGSERIAL PRIMARY KEY,
    session_id UUID NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    turn_id UUID,
    entity_type TEXT NOT NULL,
    entity_id UUID,
    event_type TEXT NOT NULL,
    status TEXT,
    payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS event_stream_session_sequence_idx ON event_stream(session_id, sequence);

CREATE TABLE IF NOT EXISTS turns (
    id UUID PRIMARY KEY,
    session_id UUID NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    role TEXT NOT NULL,
    input_text TEXT NOT NULL,
    status TEXT NOT NULL,
    started_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    completed_at TIMESTAMPTZ,
    duration_ms BIGINT,
    truncation JSONB NOT NULL DEFAULT '{}'::jsonb
);

CREATE TABLE IF NOT EXISTS model_events (
    id UUID PRIMARY KEY,
    session_id UUID NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    turn_id UUID REFERENCES turns(id) ON DELETE CASCADE,
    event_type TEXT NOT NULL,
    payload JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    ordinal BIGSERIAL NOT NULL
);

CREATE TABLE IF NOT EXISTS tool_calls (
    id UUID PRIMARY KEY,
    session_id UUID NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    turn_id UUID NOT NULL REFERENCES turns(id) ON DELETE CASCADE,
    tool_name TEXT NOT NULL,
    call_identity TEXT NOT NULL,
    input JSONB NOT NULL,
    status TEXT NOT NULL,
    result JSONB,
    started_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    completed_at TIMESTAMPTZ,
    duration_ms BIGINT,
    truncation JSONB NOT NULL DEFAULT '{}'::jsonb
);

CREATE TABLE IF NOT EXISTS script_runs (
    id UUID PRIMARY KEY,
    tool_call_id UUID NOT NULL REFERENCES tool_calls(id) ON DELETE CASCADE,
    source TEXT NOT NULL,
    status TEXT NOT NULL,
    final_output TEXT,
    stdout TEXT NOT NULL DEFAULT '',
    stderr TEXT NOT NULL DEFAULT '',
    started_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    completed_at TIMESTAMPTZ,
    duration_ms BIGINT,
    truncation JSONB NOT NULL DEFAULT '{}'::jsonb
);

CREATE TABLE IF NOT EXISTS host_api_calls (
    id UUID PRIMARY KEY,
    script_run_id UUID NOT NULL REFERENCES script_runs(id) ON DELETE CASCADE,
    api_name TEXT NOT NULL,
    input JSONB NOT NULL,
    output JSONB,
    status TEXT NOT NULL,
    started_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    completed_at TIMESTAMPTZ,
    duration_ms BIGINT,
    truncation JSONB NOT NULL DEFAULT '{}'::jsonb
);

CREATE TABLE IF NOT EXISTS command_runs (
    id UUID PRIMARY KEY,
    host_api_call_id UUID NOT NULL REFERENCES host_api_calls(id) ON DELETE CASCADE,
    binary_name TEXT NOT NULL,
    argv JSONB NOT NULL,
    cwd TEXT NOT NULL,
    stdout TEXT NOT NULL DEFAULT '',
    stderr TEXT NOT NULL DEFAULT '',
    exit_status INTEGER,
    status TEXT NOT NULL,
    started_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    completed_at TIMESTAMPTZ,
    duration_ms BIGINT,
    timeout_ms BIGINT NOT NULL,
    policy_decision JSONB NOT NULL DEFAULT '{}'::jsonb,
    truncation JSONB NOT NULL DEFAULT '{}'::jsonb
);

ALTER TABLE command_runs ADD COLUMN IF NOT EXISTS policy_decision JSONB NOT NULL DEFAULT '{}'::jsonb;

ALTER TABLE sessions ADD COLUMN IF NOT EXISTS role_id TEXT;
ALTER TABLE sessions ADD COLUMN IF NOT EXISTS role_version TEXT;
ALTER TABLE sessions ADD COLUMN IF NOT EXISTS role_snapshot JSONB NOT NULL DEFAULT '{}'::jsonb;

CREATE TABLE IF NOT EXISTS roles (
    id TEXT PRIMARY KEY,
    display_name TEXT NOT NULL,
    current_version_id UUID,
    status TEXT NOT NULL DEFAULT 'active',
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS role_versions (
    id UUID PRIMARY KEY,
    role_id TEXT NOT NULL REFERENCES roles(id) ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED,
    version TEXT NOT NULL,
    display_name TEXT NOT NULL,
    instruction_text TEXT NOT NULL,
    manifest JSONB NOT NULL,
    model_defaults JSONB NOT NULL,
    policy JSONB NOT NULL,
    routing JSONB NOT NULL,
    visibility JSONB NOT NULL,
    lifecycle_authority JSONB NOT NULL,
    snapshot JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_by TEXT NOT NULL DEFAULT 'seed-import'
);

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'roles_current_version_fk'
    ) THEN
        ALTER TABLE roles ADD CONSTRAINT roles_current_version_fk
            FOREIGN KEY (current_version_id) REFERENCES role_versions(id) DEFERRABLE INITIALLY DEFERRED;
    END IF;
END $$;
CREATE INDEX IF NOT EXISTS role_versions_role_created_idx ON role_versions(role_id, created_at DESC);

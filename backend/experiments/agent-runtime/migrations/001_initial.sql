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
ALTER TABLE command_runs ADD COLUMN IF NOT EXISTS command_version_id UUID;

CREATE TABLE IF NOT EXISTS command_definitions (
    id UUID PRIMARY KEY,
    action_id TEXT NOT NULL,
    scope_type TEXT NOT NULL DEFAULT 'global' CHECK (scope_type IN ('global', 'project')),
    project_key TEXT,
    enabled BOOLEAN NOT NULL DEFAULT true,
    current_version_id UUID,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

ALTER TABLE command_definitions DROP CONSTRAINT IF EXISTS command_definitions_action_id_key;
ALTER TABLE command_definitions ADD COLUMN IF NOT EXISTS scope_type TEXT NOT NULL DEFAULT 'global';
ALTER TABLE command_definitions ADD COLUMN IF NOT EXISTS project_key TEXT;
CREATE UNIQUE INDEX IF NOT EXISTS command_definitions_scope_unique_idx
    ON command_definitions(action_id, scope_type, COALESCE(project_key, ''));

CREATE TABLE IF NOT EXISTS command_versions (
    id UUID PRIMARY KEY,
    definition_id UUID NOT NULL REFERENCES command_definitions(id) ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED,
    version_number BIGINT NOT NULL,
    action_id TEXT NOT NULL,
    binary_name TEXT NOT NULL,
    starlark_object TEXT NOT NULL,
    starlark_method TEXT NOT NULL,
    config JSONB NOT NULL,
    model_description TEXT NOT NULL,
    created_by TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(definition_id, version_number)
);

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'command_definitions_current_version_fk'
    ) THEN
        ALTER TABLE command_definitions ADD CONSTRAINT command_definitions_current_version_fk
            FOREIGN KEY (current_version_id) REFERENCES command_versions(id) DEFERRABLE INITIALLY DEFERRED;
    END IF;
END $$;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'command_runs_command_version_fk'
    ) THEN
        ALTER TABLE command_runs ADD CONSTRAINT command_runs_command_version_fk
            FOREIGN KEY (command_version_id) REFERENCES command_versions(id) ON DELETE SET NULL;
    END IF;
END $$;

CREATE INDEX IF NOT EXISTS command_versions_action_idx ON command_versions(action_id, created_at DESC);

CREATE TABLE IF NOT EXISTS command_registry_requests (
    id UUID PRIMARY KEY,
    session_id UUID REFERENCES sessions(id) ON DELETE SET NULL,
    operation TEXT NOT NULL CHECK (operation IN ('add', 'update', 'disable', 'enable')),
    proposed_command JSONB NOT NULL,
    requester_context JSONB NOT NULL DEFAULT '{}'::jsonb,
    rationale TEXT NOT NULL,
    recommended_policy TEXT NOT NULL,
    requester TEXT NOT NULL,
    requested_by_role JSONB NOT NULL DEFAULT '{}'::jsonb,
    approval_request_id UUID,
    final_scope JSONB,
    final_execution_policy JSONB,
    final_command JSONB,
    approval_status TEXT NOT NULL CHECK (approval_status IN ('pending', 'approved', 'denied')),
    application_status TEXT NOT NULL CHECK (application_status IN ('pending', 'applied', 'failed')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    decided_at TIMESTAMPTZ,
    applied_at TIMESTAMPTZ
);
ALTER TABLE command_registry_requests ADD COLUMN IF NOT EXISTS session_id UUID REFERENCES sessions(id) ON DELETE SET NULL;
ALTER TABLE command_registry_requests ADD COLUMN IF NOT EXISTS requested_by_role JSONB NOT NULL DEFAULT '{}'::jsonb;
ALTER TABLE command_registry_requests ADD COLUMN IF NOT EXISTS approval_request_id UUID;
ALTER TABLE command_registry_requests ADD COLUMN IF NOT EXISTS requester_context JSONB NOT NULL DEFAULT '{}'::jsonb;
ALTER TABLE command_registry_requests ADD COLUMN IF NOT EXISTS final_scope JSONB;
ALTER TABLE command_registry_requests ADD COLUMN IF NOT EXISTS final_execution_policy JSONB;
ALTER TABLE command_registry_requests ADD COLUMN IF NOT EXISTS final_command JSONB;
CREATE INDEX IF NOT EXISTS command_registry_requests_status_idx ON command_registry_requests(approval_status, application_status, created_at DESC);

ALTER TABLE sessions ADD COLUMN IF NOT EXISTS role_id TEXT;
ALTER TABLE sessions ADD COLUMN IF NOT EXISTS role_version TEXT;
ALTER TABLE sessions ADD COLUMN IF NOT EXISTS role_snapshot JSONB NOT NULL DEFAULT '{}'::jsonb;
ALTER TABLE sessions ADD COLUMN IF NOT EXISTS project_key TEXT;

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

CREATE TABLE IF NOT EXISTS approval_requests (
    id UUID PRIMARY KEY,
    session_id UUID NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    turn_id UUID,
    action_name TEXT NOT NULL,
    requested_by_role JSONB NOT NULL,
    input_context JSONB NOT NULL,
    required_approver_kind TEXT NOT NULL CHECK (required_approver_kind IN ('owner', 'orchestrator')),
    status TEXT NOT NULL CHECK (status IN ('pending', 'approved', 'denied', 'expired', 'cancelled')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    completed_at TIMESTAMPTZ
);
CREATE INDEX IF NOT EXISTS approval_requests_session_created_idx ON approval_requests(session_id, created_at DESC);
CREATE INDEX IF NOT EXISTS approval_requests_status_created_idx ON approval_requests(status, created_at DESC);

CREATE TABLE IF NOT EXISTS approval_decisions (
    id UUID PRIMARY KEY,
    request_id UUID NOT NULL REFERENCES approval_requests(id) ON DELETE CASCADE,
    decision TEXT NOT NULL CHECK (decision IN ('approved', 'denied')),
    reason TEXT NOT NULL,
    decided_by JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS approval_decisions_request_created_idx ON approval_decisions(request_id, created_at ASC);

CREATE TABLE IF NOT EXISTS paused_actions (
    id UUID PRIMARY KEY,
    approval_request_id UUID NOT NULL REFERENCES approval_requests(id) ON DELETE CASCADE,
    session_id UUID NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    turn_id UUID,
    tool_call_id UUID,
    script_run_id UUID,
    action_name TEXT NOT NULL,
    action_input JSONB NOT NULL,
    role_snapshot JSONB NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('pendingApproval', 'approved', 'resuming', 'completed', 'failed', 'cancelled')),
    result JSONB,
    error JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    completed_at TIMESTAMPTZ
);
CREATE INDEX IF NOT EXISTS paused_actions_approval_request_idx ON paused_actions(approval_request_id);
CREATE INDEX IF NOT EXISTS paused_actions_session_created_idx ON paused_actions(session_id, created_at DESC);

CREATE TABLE IF NOT EXISTS file_mutations (
    id UUID PRIMARY KEY,
    script_run_id UUID NOT NULL REFERENCES script_runs(id) ON DELETE CASCADE,
    action_name TEXT NOT NULL,
    path TEXT NOT NULL,
    before_state JSONB NOT NULL,
    after_state JSONB NOT NULL,
    status TEXT NOT NULL,
    error TEXT,
    started_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    completed_at TIMESTAMPTZ,
    duration_ms BIGINT,
    policy_decision JSONB NOT NULL DEFAULT '{}'::jsonb,
    approval_request_id UUID REFERENCES approval_requests(id) ON DELETE SET NULL,
    truncation JSONB NOT NULL DEFAULT '{}'::jsonb
);
CREATE INDEX IF NOT EXISTS file_mutations_script_idx ON file_mutations(script_run_id);

CREATE TABLE IF NOT EXISTS patch_runs (
    id UUID PRIMARY KEY,
    script_run_id UUID NOT NULL REFERENCES script_runs(id) ON DELETE CASCADE,
    action_name TEXT NOT NULL,
    affected_paths JSONB NOT NULL,
    before_state JSONB NOT NULL,
    after_state JSONB NOT NULL,
    status TEXT NOT NULL,
    error TEXT,
    started_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    completed_at TIMESTAMPTZ,
    duration_ms BIGINT,
    policy_decision JSONB NOT NULL DEFAULT '{}'::jsonb,
    approval_request_id UUID REFERENCES approval_requests(id) ON DELETE SET NULL,
    truncation JSONB NOT NULL DEFAULT '{}'::jsonb
);
CREATE INDEX IF NOT EXISTS patch_runs_script_idx ON patch_runs(script_run_id);

CREATE TABLE IF NOT EXISTS sessions (
    id UUID PRIMARY KEY,
    status TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb
);
CREATE EXTENSION IF NOT EXISTS vector;

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
ALTER TABLE event_stream ALTER COLUMN session_id DROP NOT NULL;

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

CREATE TABLE IF NOT EXISTS session_context_events (
    sequence BIGSERIAL PRIMARY KEY,
    session_id UUID NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    turn_id UUID,
    event_kind TEXT NOT NULL,
    role_epoch TEXT NOT NULL,
    context_epoch BIGINT NOT NULL,
    previous_context_epoch BIGINT,
    payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS session_context_events_session_sequence_idx
    ON session_context_events(session_id, sequence);

CREATE TABLE IF NOT EXISTS session_context_snapshots (
    id UUID PRIMARY KEY,
    session_id UUID NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    turn_id UUID,
    role_epoch TEXT NOT NULL,
    context_epoch BIGINT NOT NULL,
    context_event_watermark BIGINT NOT NULL,
    snapshot JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS session_context_snapshots_session_epoch_idx
    ON session_context_snapshots(session_id, context_epoch DESC);

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
    max_runtime_ms BIGINT,
    policy_decision JSONB NOT NULL DEFAULT '{}'::jsonb,
    truncation JSONB NOT NULL DEFAULT '{}'::jsonb
);

ALTER TABLE command_runs ADD COLUMN IF NOT EXISTS policy_decision JSONB NOT NULL DEFAULT '{}'::jsonb;
ALTER TABLE command_runs ADD COLUMN IF NOT EXISTS command_version_id UUID;
ALTER TABLE command_runs ADD COLUMN IF NOT EXISTS max_runtime_ms BIGINT;

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

CREATE TABLE IF NOT EXISTS managed_processes (
    id UUID PRIMARY KEY,
    handle TEXT NOT NULL UNIQUE,
    session_id UUID NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    starting_turn_id UUID,
    command_version_id UUID REFERENCES command_versions(id) ON DELETE SET NULL,
    binary_name TEXT NOT NULL,
    argv JSONB NOT NULL,
    cwd TEXT NOT NULL,
    os_pid BIGINT,
    os_pgid BIGINT,
    status TEXT NOT NULL,
    start_time TIMESTAMPTZ NOT NULL DEFAULT now(),
    end_time TIMESTAMPTZ,
    end_of_turn_behavior TEXT NOT NULL,
    end_of_session_behavior TEXT NOT NULL DEFAULT 'block',
    max_runtime_ms BIGINT,
    termination_reason TEXT,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb
);

CREATE TABLE IF NOT EXISTS process_output_chunks (
    id UUID PRIMARY KEY,
    process_id UUID NOT NULL REFERENCES managed_processes(id) ON DELETE CASCADE,
    stream TEXT NOT NULL,
    chunk_index BIGSERIAL NOT NULL,
    content TEXT NOT NULL,
    truncated BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS managed_processes_session_handle_idx ON managed_processes(session_id, handle);
CREATE INDEX IF NOT EXISTS process_output_chunks_process_idx ON process_output_chunks(process_id, chunk_index);
ALTER TABLE managed_processes ADD COLUMN IF NOT EXISTS end_of_session_behavior TEXT NOT NULL DEFAULT 'block';

CREATE TABLE IF NOT EXISTS god_mode_grants (
    id UUID PRIMARY KEY,
    session_id UUID NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    granted_by TEXT NOT NULL,
    granted_by_kind TEXT NOT NULL DEFAULT 'operator',
    reason TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'active',
    granted_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at TIMESTAMPTZ,
    revoked_at TIMESTAMPTZ,
    revoked_by TEXT,
    revoked_reason TEXT,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb
);

CREATE INDEX IF NOT EXISTS god_mode_grants_session_idx ON god_mode_grants(session_id, status, granted_at DESC);

CREATE TABLE IF NOT EXISTS shell_runs (
    id UUID PRIMARY KEY,
    script_run_id UUID NOT NULL REFERENCES script_runs(id) ON DELETE CASCADE,
    session_id UUID NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    turn_id UUID REFERENCES turns(id) ON DELETE SET NULL,
    tool_call_id UUID REFERENCES tool_calls(id) ON DELETE SET NULL,
    god_mode_grant_id UUID REFERENCES god_mode_grants(id) ON DELETE SET NULL,
    invocation_mode TEXT NOT NULL,
    shell_path TEXT NOT NULL,
    script_hash TEXT NOT NULL,
    script_source TEXT NOT NULL,
    cwd TEXT NOT NULL,
    status TEXT NOT NULL,
    started_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    completed_at TIMESTAMPTZ,
    duration_ms BIGINT,
    stdout_artifact_id UUID,
    stderr_artifact_id UUID,
    combined_artifact_id UUID,
    process_id UUID REFERENCES managed_processes(id) ON DELETE SET NULL,
    exit_status INTEGER,
    failure TEXT,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb
);

CREATE INDEX IF NOT EXISTS shell_runs_session_idx ON shell_runs(session_id, started_at DESC);

ALTER TABLE sessions ADD COLUMN IF NOT EXISTS parent_session_id UUID REFERENCES sessions(id) ON DELETE SET NULL;
ALTER TABLE sessions ADD COLUMN IF NOT EXISTS session_kind TEXT NOT NULL DEFAULT 'source';
ALTER TABLE sessions ADD COLUMN IF NOT EXISTS hidden BOOLEAN NOT NULL DEFAULT false;
CREATE INDEX IF NOT EXISTS sessions_parent_kind_idx ON sessions(parent_session_id, session_kind, hidden);

CREATE TABLE IF NOT EXISTS requirement_sets (
    id UUID PRIMARY KEY,
    source_session_id UUID NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    title TEXT,
    canonical_set JSONB NOT NULL,
    status TEXT NOT NULL DEFAULT 'active',
    enforce_on_turns BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    deactivated_at TIMESTAMPTZ,
    outcome TEXT,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb
);
CREATE INDEX IF NOT EXISTS requirement_sets_source_active_idx ON requirement_sets(source_session_id, status, enforce_on_turns);

CREATE TABLE IF NOT EXISTS requirement_items (
    id UUID PRIMARY KEY,
    requirement_set_id UUID NOT NULL REFERENCES requirement_sets(id) ON DELETE CASCADE,
    requirement_key TEXT NOT NULL,
    statement TEXT NOT NULL,
    severity TEXT NOT NULL,
    verification_method JSONB NOT NULL DEFAULT '{}'::jsonb,
    sort_order INTEGER NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(requirement_set_id, requirement_key)
);

CREATE TABLE IF NOT EXISTS requirement_progress (
    requirement_set_id UUID NOT NULL REFERENCES requirement_sets(id) ON DELETE CASCADE,
    requirement_key TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'unresolved',
    latest_claim JSONB,
    latest_verdict JSONB,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY(requirement_set_id, requirement_key)
);

CREATE TABLE IF NOT EXISTS requirement_review_bindings (
    id UUID PRIMARY KEY,
    requirement_set_id UUID NOT NULL REFERENCES requirement_sets(id) ON DELETE CASCADE,
    source_session_id UUID NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    reviewer_session_id UUID REFERENCES sessions(id) ON DELETE SET NULL,
    status TEXT NOT NULL,
    latest_claim_packet_id UUID,
    latest_verdict_packet_id UUID,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(requirement_set_id, source_session_id)
);

CREATE TABLE IF NOT EXISTS requirement_packets (
    id UUID PRIMARY KEY,
    requirement_set_id UUID NOT NULL REFERENCES requirement_sets(id) ON DELETE CASCADE,
    source_session_id UUID NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    reviewer_session_id UUID REFERENCES sessions(id) ON DELETE SET NULL,
    turn_id UUID REFERENCES turns(id) ON DELETE SET NULL,
    packet_kind TEXT NOT NULL,
    status TEXT NOT NULL,
    payload JSONB NOT NULL,
    validation_error TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS requirement_packets_source_idx ON requirement_packets(source_session_id, created_at DESC);

CREATE TABLE IF NOT EXISTS execution_output_artifacts (
    id UUID PRIMARY KEY,
    session_id UUID REFERENCES sessions(id) ON DELETE CASCADE,
    turn_id UUID REFERENCES turns(id) ON DELETE SET NULL,
    tool_call_id UUID REFERENCES tool_calls(id) ON DELETE SET NULL,
    script_run_id UUID REFERENCES script_runs(id) ON DELETE SET NULL,
    command_run_id UUID REFERENCES command_runs(id) ON DELETE SET NULL,
    process_id UUID REFERENCES managed_processes(id) ON DELETE SET NULL,
    source_type TEXT NOT NULL,
    stream TEXT NOT NULL,
    content TEXT NOT NULL,
    byte_count BIGINT NOT NULL,
    line_count BIGINT NOT NULL,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS execution_output_artifacts_session_created_idx
    ON execution_output_artifacts(session_id, created_at DESC);
CREATE INDEX IF NOT EXISTS execution_output_artifacts_script_idx
    ON execution_output_artifacts(script_run_id, stream);
CREATE INDEX IF NOT EXISTS execution_output_artifacts_command_idx
    ON execution_output_artifacts(command_run_id, stream);
CREATE INDEX IF NOT EXISTS execution_output_artifacts_process_idx
    ON execution_output_artifacts(process_id, stream, created_at DESC);

CREATE TABLE IF NOT EXISTS compaction_checkpoints (
    id UUID PRIMARY KEY,
    session_id UUID NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    status TEXT NOT NULL CHECK (status IN ('completed', 'failed')),
    source_start_turn_id UUID REFERENCES turns(id) ON DELETE SET NULL,
    source_end_turn_id UUID REFERENCES turns(id) ON DELETE SET NULL,
    compacted_through_turn_id UUID REFERENCES turns(id) ON DELETE SET NULL,
    compacted_through_event_sequence BIGINT,
    role_epoch TEXT,
    context_epoch BIGINT,
    context_event_watermark BIGINT,
    reconstruction_metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    replacement_context TEXT NOT NULL DEFAULT '',
    summary JSONB NOT NULL DEFAULT '{}'::jsonb,
    estimate_metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    model_provider_metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    failure_info JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    completed_at TIMESTAMPTZ
);
CREATE INDEX IF NOT EXISTS compaction_checkpoints_session_completed_idx
    ON compaction_checkpoints(session_id, created_at DESC)
    WHERE status = 'completed';

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

ALTER TABLE sessions ADD COLUMN IF NOT EXISTS workdir TEXT NOT NULL DEFAULT '.';
ALTER TABLE sessions ADD COLUMN IF NOT EXISTS worktree_root TEXT;
ALTER TABLE sessions ADD COLUMN IF NOT EXISTS title TEXT;
ALTER TABLE sessions ADD COLUMN IF NOT EXISTS name TEXT;
ALTER TABLE sessions ADD COLUMN IF NOT EXISTS tracked BOOLEAN NOT NULL DEFAULT true;
ALTER TABLE sessions ADD COLUMN IF NOT EXISTS archived_at TIMESTAMPTZ;
ALTER TABLE sessions ADD COLUMN IF NOT EXISTS closed_at TIMESTAMPTZ;
ALTER TABLE sessions ADD COLUMN IF NOT EXISTS close_reason TEXT;
ALTER TABLE sessions ADD COLUMN IF NOT EXISTS forked_from_session_id UUID;
ALTER TABLE sessions ADD COLUMN IF NOT EXISTS forked_from_turn_id UUID;
ALTER TABLE sessions ADD COLUMN IF NOT EXISTS root_session_id UUID;
ALTER TABLE sessions ADD COLUMN IF NOT EXISTS fork_depth INTEGER NOT NULL DEFAULT 0;
ALTER TABLE sessions ADD COLUMN IF NOT EXISTS lineage JSONB NOT NULL DEFAULT '{}'::jsonb;

CREATE TABLE IF NOT EXISTS projects (
    project_key TEXT PRIMARY KEY,
    display_name TEXT NOT NULL,
    default_workdir TEXT NOT NULL,
    default_worktree_root TEXT NOT NULL,
    default_role_id TEXT,
    default_model TEXT NOT NULL,
    archived BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
ALTER TABLE projects ADD COLUMN IF NOT EXISTS display_name TEXT NOT NULL DEFAULT '';
ALTER TABLE projects ADD COLUMN IF NOT EXISTS default_workdir TEXT NOT NULL DEFAULT '.';
ALTER TABLE projects ADD COLUMN IF NOT EXISTS default_worktree_root TEXT NOT NULL DEFAULT '.';
ALTER TABLE projects ADD COLUMN IF NOT EXISTS default_role_id TEXT;
ALTER TABLE projects ADD COLUMN IF NOT EXISTS default_model TEXT NOT NULL DEFAULT 'gpt-5.4-mini';
ALTER TABLE projects ADD COLUMN IF NOT EXISTS archived BOOLEAN NOT NULL DEFAULT false;
ALTER TABLE projects ADD COLUMN IF NOT EXISTS created_at TIMESTAMPTZ NOT NULL DEFAULT now();
ALTER TABLE projects ADD COLUMN IF NOT EXISTS updated_at TIMESTAMPTZ NOT NULL DEFAULT now();
ALTER TABLE projects DROP COLUMN IF EXISTS tracked;
ALTER TABLE projects DROP COLUMN IF EXISTS listed;
UPDATE projects SET display_name = project_key WHERE display_name = '';
INSERT INTO projects (
    project_key, display_name, default_workdir, default_worktree_root,
    default_role_id, default_model, archived
)
SELECT DISTINCT
    s.project_key,
    initcap(replace(replace(s.project_key, '-', ' '), '_', ' ')),
    COALESCE(NULLIF(s.workdir, ''), '.'),
    COALESCE(NULLIF(s.worktree_root, ''), COALESCE(NULLIF(s.workdir, ''), '.')),
    s.role_id,
    COALESCE(s.role_snapshot->'modelDefaults'->>'model', 'gpt-5.4-mini'),
    false
FROM sessions s
WHERE NULLIF(s.project_key, '') IS NOT NULL
ON CONFLICT (project_key) DO NOTHING;
DROP INDEX IF EXISTS projects_listed_display_idx;
CREATE INDEX IF NOT EXISTS projects_lifecycle_display_idx ON projects(archived, display_name, project_key);

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
ALTER TABLE role_versions ADD COLUMN IF NOT EXISTS created_by TEXT NOT NULL DEFAULT 'seed-import';
ALTER TABLE roles ADD COLUMN IF NOT EXISTS archived_at TIMESTAMPTZ;
ALTER TABLE roles ADD COLUMN IF NOT EXISTS unarchived_at TIMESTAMPTZ;

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

CREATE TABLE IF NOT EXISTS workflow_memory_script_embeddings (
    id UUID PRIMARY KEY,
    script_run_id UUID NOT NULL REFERENCES script_runs(id) ON DELETE CASCADE,
    session_id UUID NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    project_key TEXT,
    provider TEXT NOT NULL,
    model TEXT NOT NULL,
    dimensions INTEGER NOT NULL,
    storage_type TEXT NOT NULL DEFAULT 'halfvec',
    source_hash TEXT NOT NULL,
    command_fingerprint TEXT NOT NULL,
    embedding halfvec(2560) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(script_run_id)
);
CREATE INDEX IF NOT EXISTS workflow_memory_script_embeddings_project_idx
    ON workflow_memory_script_embeddings(project_key, created_at DESC);

CREATE TABLE IF NOT EXISTS workflow_memories (
    id UUID PRIMARY KEY,
    script_run_id UUID NOT NULL REFERENCES script_runs(id) ON DELETE CASCADE,
    session_id UUID NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    scope_type TEXT NOT NULL CHECK (scope_type IN ('project', 'global')),
    project_key TEXT,
    title TEXT NOT NULL,
    reason TEXT NOT NULL,
    summary TEXT NOT NULL,
    provider TEXT NOT NULL,
    model TEXT NOT NULL,
    dimensions INTEGER NOT NULL,
    storage_type TEXT NOT NULL DEFAULT 'halfvec',
    source_hash TEXT NOT NULL,
    command_fingerprint TEXT NOT NULL,
    embedding halfvec(2560) NOT NULL,
    helpful_score DOUBLE PRECISION NOT NULL DEFAULT 0,
    promoted_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX IF NOT EXISTS workflow_memories_exact_project_unique_idx
    ON workflow_memories(scope_type, COALESCE(project_key, ''), source_hash);
CREATE INDEX IF NOT EXISTS workflow_memories_project_idx
    ON workflow_memories(scope_type, project_key, promoted_at DESC);

CREATE TABLE IF NOT EXISTS workflow_memory_events (
    id UUID PRIMARY KEY,
    session_id UUID NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    turn_id UUID,
    script_run_id UUID REFERENCES script_runs(id) ON DELETE SET NULL,
    memory_id UUID REFERENCES workflow_memories(id) ON DELETE SET NULL,
    event_type TEXT NOT NULL,
    payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS workflow_memory_events_memory_idx ON workflow_memory_events(memory_id, created_at DESC);
CREATE INDEX IF NOT EXISTS workflow_memory_events_session_idx ON workflow_memory_events(session_id, created_at DESC);

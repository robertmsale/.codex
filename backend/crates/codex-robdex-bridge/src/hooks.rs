use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::Duration,
};

use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::time::timeout;

const HOOKS_CONFIG_RELATIVE_PATH: &str = ".codex/robdex-hooks.json";
const DEFAULT_HOOK_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookEvent {
    WorkerCreate,
    WorkerArchive,
    QaCreate,
    QaArchive,
    Compaction,
}

impl HookEvent {
    pub fn wire_name(self) -> &'static str {
        match self {
            Self::WorkerCreate => "onWorkerCreate",
            Self::WorkerArchive => "onWorkerArchive",
            Self::QaCreate => "onQaCreate",
            Self::QaArchive => "onQaArchive",
            Self::Compaction => "onCompaction",
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct HookConfigDocument {
    version: u32,
    #[serde(default)]
    hooks: BTreeMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct ProjectHooks {
    pub project_root: PathBuf,
    pub version: u32,
    pub hooks: BTreeMap<String, PathBuf>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HookResult {
    #[serde(default = "default_true")]
    pub ok: bool,
    #[serde(default)]
    pub artifacts: BTreeMap<String, Value>,
    #[serde(default)]
    pub prompt_append: Vec<String>,
    #[serde(default)]
    pub cleanup: Option<Value>,
    #[serde(default)]
    pub metadata: Option<Value>,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HookLifecycleState {
    #[serde(default)]
    pub branch_name: Option<String>,
    #[serde(default)]
    pub worktree_path: Option<String>,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub stack_name: Option<String>,
    #[serde(default)]
    pub artifacts: BTreeMap<String, Value>,
    #[serde(default)]
    pub cleanup: Option<Value>,
    #[serde(default)]
    pub metadata: Option<Value>,
    #[serde(default)]
    pub prompt_append: Vec<String>,
}

impl HookLifecycleState {
    pub fn from_hook_result(hook_result: &HookResult) -> Self {
        Self {
            branch_name: artifact_string(&hook_result.artifacts, "branchName"),
            worktree_path: artifact_string(&hook_result.artifacts, "worktreePath"),
            base_url: artifact_string(&hook_result.artifacts, "baseUrl"),
            stack_name: artifact_string(&hook_result.artifacts, "stackName"),
            artifacts: hook_result.artifacts.clone(),
            cleanup: hook_result.cleanup.clone(),
            metadata: hook_result.metadata.clone(),
            prompt_append: hook_result.prompt_append.clone(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HookTelemetry {
    pub event: String,
    pub status: String,
    #[serde(default)]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct HookInvocation {
    pub result: Option<HookResult>,
    pub telemetry: Option<HookTelemetry>,
}

pub async fn load_project_hooks(project_root: &str) -> Result<Option<ProjectHooks>> {
    let project_root = PathBuf::from(project_root);
    let config_path = project_root.join(HOOKS_CONFIG_RELATIVE_PATH);
    if !config_path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(&config_path)
        .with_context(|| format!("read hook config {}", config_path.display()))?;
    let parsed: HookConfigDocument = serde_json::from_str(&raw)
        .with_context(|| format!("parse hook config {}", config_path.display()))?;
    let mut hooks = BTreeMap::new();
    for (event, relative_path) in parsed.hooks {
        let resolved = resolve_hook_path(&project_root, &relative_path)?;
        hooks.insert(event, resolved);
    }
    Ok(Some(ProjectHooks {
        project_root,
        version: parsed.version,
        hooks,
    }))
}

pub async fn maybe_run_project_hook(
    project_root: &str,
    event: HookEvent,
    payload: Value,
) -> HookInvocation {
    maybe_run_project_hook_with_timeout(project_root, event, payload, DEFAULT_HOOK_TIMEOUT).await
}

async fn maybe_run_project_hook_with_timeout(
    project_root: &str,
    event: HookEvent,
    payload: Value,
    timeout_duration: Duration,
) -> HookInvocation {
    let hooks = match load_project_hooks(project_root).await {
        Ok(value) => value,
        Err(error) => {
            return HookInvocation {
                result: None,
                telemetry: Some(HookTelemetry {
                    event: event.wire_name().to_string(),
                    status: "failed".to_string(),
                    detail: Some(error.to_string()),
                }),
            };
        }
    };
    let Some(hooks) = hooks else {
        return HookInvocation::default();
    };
    let Some(script_path) = hooks.hooks.get(event.wire_name()) else {
        return HookInvocation::default();
    };
    let payload_bytes = match serde_json::to_vec(&payload) {
        Ok(bytes) => bytes,
        Err(error) => {
            return HookInvocation {
                result: None,
                telemetry: Some(HookTelemetry {
                    event: event.wire_name().to_string(),
                    status: "failed".to_string(),
                    detail: Some(error.to_string()),
                }),
            };
        }
    };
    let script_path = script_path.clone();
    let cwd = hooks.project_root.clone();
    let event_name = event.wire_name().to_string();
    let join = tokio::task::spawn_blocking(move || -> Result<HookResult> {
        let mut child = Command::new(&script_path)
            .current_dir(&cwd)
            .env("ROBDEX_HOOK_EVENT", &event_name)
            .env("ROBDEX_PROJECT_ROOT", cwd.display().to_string())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .with_context(|| format!("spawn hook {}", script_path.display()))?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(&payload_bytes)?;
        }
        let output = child
            .wait_with_output()
            .with_context(|| format!("wait for hook {}", script_path.display()))?;
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if !output.status.success() {
            bail!(
                "hook {} failed with exit code {:?}: {}",
                script_path.display(),
                output.status.code(),
                if stderr.is_empty() { stdout.as_str() } else { stderr.as_str() }
            );
        }
        if stdout.is_empty() {
            bail!("hook {} returned empty stdout", script_path.display());
        }
        let result: HookResult = serde_json::from_str(&stdout)
            .with_context(|| format!("parse hook output from {}", script_path.display()))?;
        validate_hook_result(&result)?;
        if !result.ok {
            bail!(
                "{}",
                result
                    .error
                    .clone()
                    .unwrap_or_else(|| "hook returned ok=false".to_string())
            );
        }
        Ok(result)
    });

    match timeout(timeout_duration, join).await {
        Ok(joined) => match joined {
            Ok(Ok(result)) => HookInvocation {
                result: Some(result),
                telemetry: None,
            },
            Ok(Err(error)) => HookInvocation {
                result: None,
                telemetry: Some(HookTelemetry {
                    event: event.wire_name().to_string(),
                    status: "failed".to_string(),
                    detail: Some(error.to_string()),
                }),
            },
            Err(error) => HookInvocation {
                result: None,
                telemetry: Some(HookTelemetry {
                    event: event.wire_name().to_string(),
                    status: "failed".to_string(),
                    detail: Some(anyhow!(error.to_string()).to_string()),
                }),
            },
        },
        Err(_) => HookInvocation {
            result: None,
            telemetry: Some(HookTelemetry {
                event: event.wire_name().to_string(),
                status: "timed_out".to_string(),
                detail: Some(format!(
                    "hook {} timed out after {}s",
                    event.wire_name(),
                    timeout_duration.as_secs()
                )),
            }),
        },
    }
}

pub fn default_worker_branch_name(agent_name: &str) -> String {
    format!("codex/{}", kebab_case(agent_name))
}

pub fn default_worker_worktree_path(project_root: &str, agent_name: &str) -> String {
    let root = Path::new(project_root);
    root.join(".worktrees")
        .join(kebab_case(agent_name))
        .display()
        .to_string()
}

pub fn append_prompt_segments(base_prompt: &str, extra_segments: &[String]) -> String {
    let mut segments = Vec::new();
    let trimmed = base_prompt.trim();
    if !trimmed.is_empty() {
        segments.push(trimmed.to_string());
    }
    for segment in extra_segments {
        let trimmed = segment.trim();
        if !trimmed.is_empty() {
            segments.push(trimmed.to_string());
        }
    }
    segments.join("\n\n")
}

pub fn worker_create_payload(
    thread_id: Option<&str>,
    project_id: &str,
    project_name: &str,
    project_root: &str,
    agent_name: &str,
    role: &str,
    requested_cwd: &str,
    parent_thread_id: Option<&str>,
    spawn: Value,
) -> Value {
    let mut payload = json!({
        "event": HookEvent::WorkerCreate.wire_name(),
        "project": {
            "id": project_id,
            "name": project_name,
            "root": project_root,
        },
        "projectRoot": project_root,
        "requestedCwd": requested_cwd,
        "agent": {
            "name": agent_name,
            "role": role,
        },
        "spawn": spawn,
        "defaults": {
            "branchName": default_worker_branch_name(agent_name),
            "worktreePath": default_worker_worktree_path(project_root, agent_name),
        }
    });
    if let Some(thread_id) = thread_id.filter(|value| !value.trim().is_empty()) {
        payload["threadId"] = Value::String(thread_id.to_string());
    }
    if let Some(parent_thread_id) = parent_thread_id.filter(|value| !value.trim().is_empty()) {
        payload["parentThreadId"] = Value::String(parent_thread_id.to_string());
    }
    payload
}

pub fn worker_archive_payload(
    thread_id: &str,
    project_id: &str,
    project_name: &str,
    project_root: &str,
    agent_name: &str,
    role: &str,
    requested_cwd: Option<&str>,
    lifecycle: Option<Value>,
) -> Value {
    let mut payload = json!({
        "event": HookEvent::WorkerArchive.wire_name(),
        "project": {
            "id": project_id,
            "name": project_name,
            "root": project_root,
        },
        "projectRoot": project_root,
        "threadId": thread_id,
        "agent": {
            "name": agent_name,
            "role": role,
        },
        "lifecycle": lifecycle,
    });
    if let Some(requested_cwd) = requested_cwd.filter(|value| !value.trim().is_empty()) {
        payload["requestedCwd"] = Value::String(requested_cwd.to_string());
    }
    payload
}

pub fn qa_create_payload(
    thread_id: Option<&str>,
    project_id: &str,
    project_name: &str,
    project_root: &str,
    agent_name: &str,
    role: &str,
    requested_cwd: &str,
    parent_thread_id: Option<&str>,
    spawn: Value,
) -> Value {
    let mut payload = json!({
        "event": HookEvent::QaCreate.wire_name(),
        "project": {
            "id": project_id,
            "name": project_name,
            "root": project_root,
        },
        "projectRoot": project_root,
        "requestedCwd": requested_cwd,
        "agent": {
            "name": agent_name,
            "role": role,
        },
        "spawn": spawn,
    });
    if let Some(thread_id) = thread_id.filter(|value| !value.trim().is_empty()) {
        payload["threadId"] = Value::String(thread_id.to_string());
    }
    if let Some(parent_thread_id) = parent_thread_id.filter(|value| !value.trim().is_empty()) {
        payload["parentThreadId"] = Value::String(parent_thread_id.to_string());
    }
    payload
}

pub fn qa_archive_payload(
    thread_id: &str,
    project_id: &str,
    project_name: &str,
    project_root: &str,
    agent_name: &str,
    role: &str,
    requested_cwd: Option<&str>,
    lifecycle: Option<Value>,
) -> Value {
    let mut payload = json!({
        "event": HookEvent::QaArchive.wire_name(),
        "project": {
            "id": project_id,
            "name": project_name,
            "root": project_root,
        },
        "projectRoot": project_root,
        "threadId": thread_id,
        "agent": {
            "name": agent_name,
            "role": role,
        },
        "lifecycle": lifecycle,
    });
    if let Some(requested_cwd) = requested_cwd.filter(|value| !value.trim().is_empty()) {
        payload["requestedCwd"] = Value::String(requested_cwd.to_string());
    }
    payload
}

pub fn compaction_payload(
    thread_id: &str,
    project_id: &str,
    project_name: &str,
    project_root: &str,
    agent_name: &str,
    role: &str,
    requested_cwd: Option<&str>,
    compaction_count: u64,
) -> Value {
    let mut payload = json!({
        "event": HookEvent::Compaction.wire_name(),
        "project": {
            "id": project_id,
            "name": project_name,
            "root": project_root,
        },
        "projectRoot": project_root,
        "threadId": thread_id,
        "agent": {
            "name": agent_name,
            "role": role,
        },
        "compaction": {
            "count": compaction_count,
        },
    });
    if let Some(requested_cwd) = requested_cwd.filter(|value| !value.trim().is_empty()) {
        payload["requestedCwd"] = Value::String(requested_cwd.to_string());
    }
    payload
}

fn resolve_hook_path(project_root: &Path, configured_path: &str) -> Result<PathBuf> {
    let path = PathBuf::from(configured_path);
    let resolved = if path.is_absolute() {
        path
    } else {
        project_root.join(path)
    };
    if !resolved.is_absolute() {
        return Ok(resolved);
    }
    if !PathBuf::from(configured_path).is_absolute() {
        let normalized_root = normalize_path(project_root);
        let normalized_resolved = normalize_path(&resolved);
        if !normalized_resolved.starts_with(&normalized_root) {
            bail!("hook path escapes project root");
        }
        return Ok(normalized_resolved);
    }
    Ok(resolved)
}

fn normalize_path(path: &Path) -> PathBuf {
    use std::path::Component;

    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
}

fn validate_hook_result(result: &HookResult) -> Result<()> {
    for entry in &result.prompt_append {
        if entry.trim().is_empty() {
            bail!("hook promptAppend entries must not be empty");
        }
    }
    if let Some(value) = result.artifacts.get("branchName") {
        match value {
            Value::String(text) if !text.trim().is_empty() => {}
            _ => bail!("hook artifacts.branchName must be a non-empty string"),
        }
    }
    if let Some(value) = result.artifacts.get("worktreePath") {
        match value {
            Value::String(text) if !text.trim().is_empty() => {}
            _ => bail!("hook artifacts.worktreePath must be a non-empty string"),
        }
    }
    if let Some(value) = result.cleanup.as_ref() {
        if !value.is_object() {
            bail!("hook cleanup must be an object when present");
        }
    }
    Ok(())
}

fn kebab_case(value: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    out.trim_matches('-').to_string()
}

fn default_true() -> bool {
    true
}

fn artifact_string(artifacts: &BTreeMap<String, Value>, key: &str) -> Option<String> {
    artifacts
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    use tempfile::tempdir;

    use super::*;

    fn write_executable(path: &Path, content: &str) {
        fs::write(path, content).expect("write file");
        let mut perms = fs::metadata(path).expect("meta").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path, perms).expect("chmod");
    }

    #[tokio::test]
    async fn loads_hook_config_relative_to_project_root() {
        let temp = tempdir().expect("tempdir");
        let config_dir = temp.path().join(".codex");
        fs::create_dir_all(config_dir.join("hooks")).expect("mkdirs");
        fs::write(
            config_dir.join("robdex-hooks.json"),
            r#"{"version":1,"hooks":{"onWorkerCreate":"./.codex/hooks/on-worker-create"}}"#,
        )
        .expect("write config");

        let hooks = load_project_hooks(&temp.path().display().to_string())
            .await
            .expect("hooks")
            .expect("present");
        assert_eq!(hooks.version, 1);
        assert_eq!(
            hooks.hooks.get("onWorkerCreate"),
            Some(&temp.path().join(".codex/hooks/on-worker-create"))
        );
    }

    #[tokio::test]
    async fn runs_worker_create_hook_and_parses_prompt_append() {
        let temp = tempdir().expect("tempdir");
        let config_dir = temp.path().join(".codex");
        fs::create_dir_all(config_dir.join("hooks")).expect("mkdirs");
        let hook_path = config_dir.join("hooks/on-worker-create");
        write_executable(
            &hook_path,
            "#!/bin/bash\ncat >/dev/null\necho '{\"ok\":true,\"promptAppend\":[\"Your worktree is ready.\"],\"artifacts\":{\"branchName\":\"codex/test-worker\"}}'\n",
        );
        fs::write(
            config_dir.join("robdex-hooks.json"),
            r#"{"version":1,"hooks":{"onWorkerCreate":"./.codex/hooks/on-worker-create"}}"#,
        )
        .expect("write config");

        let invocation = maybe_run_project_hook(
            &temp.path().display().to_string(),
            HookEvent::WorkerCreate,
            json!({"event":"onWorkerCreate"}),
        )
        .await;
        assert!(invocation.telemetry.is_none());
        let result = invocation.result.expect("present");

        assert_eq!(result.prompt_append, vec!["Your worktree is ready.".to_string()]);
        assert_eq!(
            result.artifacts.get("branchName"),
            Some(&Value::String("codex/test-worker".to_string()))
        );
    }

    #[tokio::test]
    async fn missing_hook_config_returns_default_invocation() {
        let temp = tempdir().expect("tempdir");
        let invocation = maybe_run_project_hook(
            &temp.path().display().to_string(),
            HookEvent::WorkerCreate,
            json!({"event":"onWorkerCreate"}),
        )
        .await;
        assert_eq!(invocation, HookInvocation::default());
    }

    #[tokio::test]
    async fn malformed_hook_config_returns_failed_telemetry() {
        let temp = tempdir().expect("tempdir");
        let config_dir = temp.path().join(".codex");
        fs::create_dir_all(&config_dir).expect("mkdirs");
        fs::write(config_dir.join("robdex-hooks.json"), "{not-json").expect("write config");

        let invocation = maybe_run_project_hook(
            &temp.path().display().to_string(),
            HookEvent::WorkerCreate,
            json!({"event":"onWorkerCreate"}),
        )
        .await;

        assert!(invocation.result.is_none());
        let telemetry = invocation.telemetry.expect("telemetry");
        assert_eq!(telemetry.status, "failed");
        assert!(telemetry.detail.unwrap_or_default().contains("parse hook config"));
    }

    #[tokio::test]
    async fn nonzero_hook_exit_returns_failed_telemetry() {
        let temp = tempdir().expect("tempdir");
        let config_dir = temp.path().join(".codex");
        fs::create_dir_all(config_dir.join("hooks")).expect("mkdirs");
        let hook_path = config_dir.join("hooks/on-worker-create");
        write_executable(&hook_path, "#!/bin/bash\ncat >/dev/null\necho boom >&2\nexit 7\n");
        fs::write(
            config_dir.join("robdex-hooks.json"),
            r#"{"version":1,"hooks":{"onWorkerCreate":"./.codex/hooks/on-worker-create"}}"#,
        )
        .expect("write config");

        let invocation = maybe_run_project_hook(
            &temp.path().display().to_string(),
            HookEvent::WorkerCreate,
            json!({"event":"onWorkerCreate"}),
        )
        .await;

        assert!(invocation.result.is_none());
        let telemetry = invocation.telemetry.expect("telemetry");
        assert_eq!(telemetry.status, "failed");
        assert_eq!(telemetry.event, "onWorkerCreate");
        assert!(telemetry.detail.unwrap_or_default().contains("exit code"));
    }

    #[tokio::test]
    async fn invalid_hook_output_returns_failed_telemetry() {
        let temp = tempdir().expect("tempdir");
        let config_dir = temp.path().join(".codex");
        fs::create_dir_all(config_dir.join("hooks")).expect("mkdirs");
        let hook_path = config_dir.join("hooks/on-worker-create");
        write_executable(&hook_path, "#!/bin/bash\ncat >/dev/null\necho '{\"ok\":true,\"artifacts\":{\"branchName\":123}}'\n");
        fs::write(
            config_dir.join("robdex-hooks.json"),
            r#"{"version":1,"hooks":{"onWorkerCreate":"./.codex/hooks/on-worker-create"}}"#,
        )
        .expect("write config");

        let invocation = maybe_run_project_hook(
            &temp.path().display().to_string(),
            HookEvent::WorkerCreate,
            json!({"event":"onWorkerCreate"}),
        )
        .await;

        assert!(invocation.result.is_none());
        let telemetry = invocation.telemetry.expect("telemetry");
        assert_eq!(telemetry.status, "failed");
        assert!(
            telemetry
                .detail
                .unwrap_or_default()
                .contains("artifacts.branchName")
        );
    }

    #[tokio::test]
    async fn hook_timeout_returns_timed_out_telemetry() {
        let temp = tempdir().expect("tempdir");
        let config_dir = temp.path().join(".codex");
        fs::create_dir_all(config_dir.join("hooks")).expect("mkdirs");
        let hook_path = config_dir.join("hooks/on-worker-create");
        write_executable(&hook_path, "#!/bin/bash\ncat >/dev/null\nsleep 1\necho '{\"ok\":true}'\n");
        fs::write(
            config_dir.join("robdex-hooks.json"),
            r#"{"version":1,"hooks":{"onWorkerCreate":"./.codex/hooks/on-worker-create"}}"#,
        )
        .expect("write config");

        let invocation = maybe_run_project_hook_with_timeout(
            &temp.path().display().to_string(),
            HookEvent::WorkerCreate,
            json!({"event":"onWorkerCreate"}),
            Duration::from_millis(10),
        )
        .await;

        assert!(invocation.result.is_none());
        let telemetry = invocation.telemetry.expect("telemetry");
        assert_eq!(telemetry.status, "timed_out");
        assert!(telemetry.detail.unwrap_or_default().contains("timed out"));
    }

    #[tokio::test]
    async fn runs_qa_create_hook() {
        let temp = tempdir().expect("tempdir");
        let config_dir = temp.path().join(".codex");
        fs::create_dir_all(config_dir.join("hooks")).expect("mkdirs");
        let hook_path = config_dir.join("hooks/on-qa-create");
        write_executable(
            &hook_path,
            "#!/bin/bash\ncat >/dev/null\necho '{\"ok\":true,\"artifacts\":{\"baseUrl\":\"http://127.0.0.1:54136\"},\"promptAppend\":[\"QA lane prepared.\"]}'\n",
        );
        fs::write(
            config_dir.join("robdex-hooks.json"),
            r#"{"version":1,"hooks":{"onQaCreate":"./.codex/hooks/on-qa-create"}}"#,
        )
        .expect("write config");

        let invocation = maybe_run_project_hook(
            &temp.path().display().to_string(),
            HookEvent::QaCreate,
            json!({"event":"onQaCreate"}),
        )
        .await;

        assert!(invocation.telemetry.is_none());
        let result = invocation.result.expect("present");
        assert_eq!(result.prompt_append, vec!["QA lane prepared.".to_string()]);
        assert_eq!(
            result.artifacts.get("baseUrl"),
            Some(&Value::String("http://127.0.0.1:54136".to_string()))
        );
    }

    #[test]
    fn worker_defaults_use_kebab_case_names() {
        assert_eq!(
            default_worker_branch_name("Worker QA Workflow Print View"),
            "codex/worker-qa-workflow-print-view"
        );
        assert_eq!(
            default_worker_worktree_path("/tmp/project", "Worker QA Workflow Print View"),
            "/tmp/project/.worktrees/worker-qa-workflow-print-view"
        );
    }

    #[test]
    fn appends_prompt_segments_cleanly() {
        assert_eq!(
            append_prompt_segments("Base prompt", &["Extra one".to_string(), "Extra two".to_string()]),
            "Base prompt\n\nExtra one\n\nExtra two"
        );
    }

    #[test]
    fn validates_known_hook_fields() {
        let result = HookResult {
            ok: true,
            artifacts: BTreeMap::from([
                ("branchName".to_string(), Value::String("codex/test".to_string())),
                ("worktreePath".to_string(), Value::String("/tmp/worktree".to_string())),
            ]),
            prompt_append: vec!["Use this worktree.".to_string()],
            cleanup: Some(json!({"onArchive": true})),
            metadata: None,
            error: None,
        };
        validate_hook_result(&result).expect("valid hook result");
    }

    #[test]
    fn resolves_lifecycle_state_from_common_artifacts() {
        let hook_result = HookResult {
            ok: true,
            artifacts: BTreeMap::from([
                ("branchName".to_string(), Value::String("codex/test".to_string())),
                ("worktreePath".to_string(), Value::String("/tmp/worktree".to_string())),
                ("baseUrl".to_string(), Value::String("http://127.0.0.1:1234".to_string())),
                ("stackName".to_string(), Value::String("worker-stack".to_string())),
                ("custom".to_string(), json!({"value": true})),
            ]),
            prompt_append: vec!["Use this worktree.".to_string()],
            cleanup: Some(json!({"onArchive": true})),
            metadata: Some(json!({"device": "sim-1"})),
            error: None,
        };

        let lifecycle = HookLifecycleState::from_hook_result(&hook_result);
        assert_eq!(lifecycle.branch_name.as_deref(), Some("codex/test"));
        assert_eq!(lifecycle.worktree_path.as_deref(), Some("/tmp/worktree"));
        assert_eq!(lifecycle.base_url.as_deref(), Some("http://127.0.0.1:1234"));
        assert_eq!(lifecycle.stack_name.as_deref(), Some("worker-stack"));
        assert_eq!(lifecycle.artifacts.get("custom"), Some(&json!({"value": true})));
        assert_eq!(lifecycle.cleanup, Some(json!({"onArchive": true})));
        assert_eq!(lifecycle.metadata, Some(json!({"device": "sim-1"})));
    }

    #[test]
    fn resolve_hook_path_rejects_escape() {
        let project_root = PathBuf::from("/tmp/project");
        let error = resolve_hook_path(&project_root, "../outside").expect_err("path escape should fail");
        assert!(error.to_string().contains("escapes project root"));
    }
}

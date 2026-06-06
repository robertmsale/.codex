use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::{
    commands::{
        PersistedAgentState, PersistedState, RequirementSetState, RequirementState, archive_thread,
        orchestrator_spawn_agent, parse_state, persist_state, unix_now,
    },
    runtime::BridgeRuntime,
};

const PROJECT_MANIFEST_RUNS_KEY: &str = "manifestRuns";
const MANIFEST_BINDING_KEY: &str = "manifestBinding";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ManifestSpec {
    pub schema_version: u32,
    pub plan_id: String,
    pub title: String,
    pub phases: Vec<ManifestPhaseSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ManifestPhaseSpec {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub prompt: String,
    #[serde(default)]
    pub requirements: Vec<RequirementState>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ManifestRunState {
    pub run_id: String,
    pub plan_id: String,
    pub title: String,
    pub schema_version: u32,
    pub project_path: String,
    pub source_path: String,
    pub source_hash: String,
    pub source_snapshot: String,
    pub rationale_markdown: String,
    pub status: String,
    pub current_phase_id: Option<String>,
    pub phases: Vec<ManifestPhaseRunState>,
    #[serde(default)]
    pub audit_events: Vec<ManifestAuditEvent>,
    #[serde(default)]
    pub activated_at: u64,
    #[serde(default)]
    pub updated_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ManifestPhaseRunState {
    pub phase_id: String,
    pub title: String,
    pub status: String,
    #[serde(default)]
    pub prompt: String,
    #[serde(default)]
    pub requirements: Vec<RequirementState>,
    #[serde(default)]
    pub worker_thread_id: Option<String>,
    #[serde(default)]
    pub handoff: Option<ManifestHandoffState>,
    #[serde(default)]
    pub durable_evidence: Option<Value>,
    #[serde(default)]
    pub blocker: Option<Value>,
    #[serde(default)]
    pub waiver: Option<Value>,
    #[serde(default)]
    pub resume_decision: Option<Value>,
    #[serde(default)]
    pub archive_cleanup_state: String,
    #[serde(default)]
    pub archive_safe: bool,
    #[serde(default)]
    pub materialized_at: Option<u64>,
    #[serde(default)]
    pub passed_at: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ManifestHandoffState {
    pub text: String,
    pub captured_at: u64,
    #[serde(default)]
    pub source_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ManifestAuditEvent {
    pub event: String,
    pub at: u64,
    #[serde(default)]
    pub phase_id: Option<String>,
    #[serde(default)]
    pub worker_thread_id: Option<String>,
    #[serde(default)]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ManifestBindingState {
    pub run_id: String,
    pub plan_id: String,
    pub phase_id: String,
    #[serde(default)]
    pub archive_safe: bool,
}

pub async fn manifest_activate(
    runtime: &BridgeRuntime,
    sender_thread_id: &str,
    file_path: &str,
) -> Result<Value> {
    let loaded = load_manifest_file(file_path)?;
    let project_path = project_path_for_manifest(&loaded.source_path)?;
    let run_id = format!("manifest-{}-{}", loaded.spec.plan_id, unix_now());
    let mut run = ManifestRunState {
        run_id: run_id.clone(),
        plan_id: loaded.spec.plan_id.clone(),
        title: loaded.spec.title.clone(),
        schema_version: loaded.spec.schema_version,
        project_path: project_path.display().to_string(),
        source_path: loaded.source_path.display().to_string(),
        source_hash: loaded.source_hash.clone(),
        source_snapshot: loaded.source_snapshot,
        rationale_markdown: loaded.rationale_markdown,
        status: "active".to_string(),
        current_phase_id: loaded.spec.phases.first().map(|phase| phase.id.clone()),
        phases: loaded
            .spec
            .phases
            .into_iter()
            .map(|phase| ManifestPhaseRunState {
                phase_id: phase.id,
                title: phase.title,
                status: "ghost".to_string(),
                prompt: phase.prompt,
                requirements: phase.requirements,
                worker_thread_id: None,
                handoff: None,
                durable_evidence: None,
                blocker: None,
                waiver: None,
                resume_decision: None,
                archive_cleanup_state: "notReady".to_string(),
                archive_safe: false,
                materialized_at: None,
                passed_at: None,
            })
            .collect(),
        audit_events: vec![ManifestAuditEvent {
            event: "activated".to_string(),
            at: unix_now(),
            phase_id: None,
            worker_thread_id: None,
            detail: Some("source file read, hashed, and snapshotted".to_string()),
        }],
        activated_at: unix_now(),
        updated_at: unix_now(),
    };
    let project_key = {
        let _guard = runtime.lock_state_mutation().await;
        let mut state = parse_state(&runtime.state_document_value().await);
        let project_key = project_key_by_root(&state, project_path.to_string_lossy().as_ref())
            .ok_or_else(|| anyhow::anyhow!("Project `{}` is not tracked by Robdex.", project_path.display()))?;
        ensure_no_active_conflict(&state, &project_key, loaded.spec.plan_id.as_str(), loaded.source_hash.as_str())?;
        insert_manifest_run(&mut state, &project_key, run.clone())?;
        persist_state(runtime, &state).await?;
        project_key
    };
    let mut state = parse_state(&runtime.state_document_value().await);
    let materialized = materialize_current_phase(runtime, sender_thread_id, &mut state, &project_key, &mut run, None).await;
    let (phase_id, worker_thread_id) = match materialized {
        Ok(value) => value,
        Err(error) => {
            let mut failed_state = parse_state(&runtime.state_document_value().await);
            let mut failed_run = get_manifest_run(&failed_state, &project_key, &run_id)?;
            failed_run.status = "blocked".to_string();
            failed_run.audit_events.push(ManifestAuditEvent {
                event: "materializationUncertainty".to_string(),
                at: unix_now(),
                phase_id: failed_run.current_phase_id.clone(),
                worker_thread_id: None,
                detail: Some(error.to_string()),
            });
            failed_run.updated_at = unix_now();
            replace_manifest_run(&mut failed_state, &project_key, failed_run)?;
            persist_state(runtime, &failed_state).await?;
            return Err(error);
        }
    };
    replace_manifest_run(&mut state, &project_key, run.clone())?;
    persist_state(runtime, &state).await?;
    Ok(json!({
        "run": run,
        "runId": run_id,
        "currentPhaseId": phase_id,
        "workerThreadId": worker_thread_id,
    }))
}

pub async fn manifest_decision(
    runtime: &BridgeRuntime,
    sender_thread_id: &str,
    run_id: &str,
    phase_id: &str,
    decision_type: &str,
    text: &str,
) -> Result<Value> {
    let decision = text.trim();
    if decision.is_empty() {
        bail!("Manifest decision text is required.");
    }
    let mut state = parse_state(&runtime.state_document_value().await);
    let project_key = project_key_for_thread(&state, sender_thread_id)
        .or_else(|| project_key_for_run(&state, run_id))
        .ok_or_else(|| anyhow::anyhow!("No tracked project found for manifest decision."))?;
    let mut run = get_manifest_run(&state, &project_key, run_id)?;
    let phase = run
        .phases
        .iter_mut()
        .find(|phase| phase.phase_id == phase_id)
        .ok_or_else(|| anyhow::anyhow!("Manifest phase `{phase_id}` was not found."))?;
    let payload = json!({
        "type": decision_type,
        "text": decision,
        "recordedAt": unix_now(),
        "senderThreadId": sender_thread_id,
    });
    let event = match decision_type {
        "blocker" => {
            phase.blocker = Some(payload);
            "blockerRecorded"
        }
        "waiver" => {
            phase.waiver = Some(payload);
            "waiverRecorded"
        }
        "resume" => {
            phase.resume_decision = Some(payload);
            "resumeDecisionRecorded"
        }
        other => bail!("Unsupported manifest decision type `{other}`; expected blocker, waiver, or resume."),
    };
    run.audit_events.push(ManifestAuditEvent {
        event: event.to_string(),
        at: unix_now(),
        phase_id: Some(phase_id.to_string()),
        worker_thread_id: phase.worker_thread_id.clone(),
        detail: Some(decision.to_string()),
    });
    run.updated_at = unix_now();
    replace_manifest_run(&mut state, &project_key, run.clone())?;
    persist_state(runtime, &state).await?;
    Ok(json!({ "run": run }))
}

pub async fn manifest_status(
    runtime: &BridgeRuntime,
    sender_thread_id: &str,
    project_path: Option<&str>,
    run_id: Option<&str>,
) -> Result<Value> {
    let state = parse_state(&runtime.state_document_value().await);
    let project_key = if let Some(path) = project_path {
        project_key_by_root(&state, path)
            .ok_or_else(|| anyhow::anyhow!("Project `{path}` is not tracked by Robdex."))?
    } else {
        project_key_for_thread(&state, sender_thread_id)
            .ok_or_else(|| anyhow::anyhow!("Thread `{sender_thread_id}` is not tracked by Robdex."))?
    };
    let mut runs = manifest_runs_for_project(&state, &project_key)?;
    if let Some(run_id) = run_id {
        runs.retain(|run| run.run_id == run_id);
        if runs.is_empty() {
            bail!("Manifest run `{run_id}` was not found.");
        }
    }
    Ok(json!({ "runs": runs }))
}

pub async fn manifest_advance(
    runtime: &BridgeRuntime,
    sender_thread_id: &str,
    run_id: &str,
    handoff_path: &str,
) -> Result<Value> {
    let handoff = fs::read_to_string(handoff_path)
        .with_context(|| format!("failed to read handoff file {handoff_path}"))?;
    if handoff.trim().is_empty() {
        bail!("Handoff file is empty; manifest advance requires a durable handoff.");
    }
    let reservation = {
        let _guard = runtime.lock_state_mutation().await;
        let mut state = parse_state(&runtime.state_document_value().await);
        let project_key = project_key_for_thread(&state, sender_thread_id)
            .or_else(|| project_key_for_run(&state, run_id))
            .ok_or_else(|| anyhow::anyhow!("No tracked project found for manifest advance."))?;
        let reservation = reserve_manifest_advance_transition(
            &mut state,
            &project_key,
            run_id,
            &handoff,
            Path::new(handoff_path).display().to_string(),
        )?;
        persist_state(runtime, &state).await?;
        reservation
    };

    let mut next_worker_thread_id = None;
    if let Some(next_phase_id) = reservation.next_phase_id.clone() {
        let mut state = parse_state(&runtime.state_document_value().await);
        let mut run = get_manifest_run(&state, &reservation.project_key, run_id)?;
        match materialize_reserved_phase(
            runtime,
            sender_thread_id,
            &mut state,
            &reservation.project_key,
            &mut run,
            &next_phase_id,
            reservation.phase_index,
            &reservation.worker_thread_id,
        )
        .await
        {
            Ok(materialized_worker) => {
                next_worker_thread_id = Some(materialized_worker);
            }
            Err(error) => {
                let _guard = runtime.lock_state_mutation().await;
                let mut failed_state = parse_state(&runtime.state_document_value().await);
                let mut failed_run = get_manifest_run(&failed_state, &reservation.project_key, run_id)?;
                failed_run.status = "blocked".to_string();
                failed_run.audit_events.push(ManifestAuditEvent {
                    event: "materializationUncertainty".to_string(),
                    at: unix_now(),
                    phase_id: Some(next_phase_id),
                    worker_thread_id: None,
                    detail: Some(error.to_string()),
                });
                failed_run.updated_at = unix_now();
                replace_manifest_run(&mut failed_state, &reservation.project_key, failed_run)?;
                persist_state(runtime, &failed_state).await?;
                return Err(error);
            }
        }
    }

    let archive_result = archive_thread(runtime, &reservation.worker_thread_id).await;
    let _guard = runtime.lock_state_mutation().await;
    let mut post_archive_state = parse_state(&runtime.state_document_value().await);
    let mut post_archive_run = get_manifest_run(&post_archive_state, &reservation.project_key, run_id)?;
    let cleanup_state = match archive_result {
        Ok(()) => "archived",
        Err(ref error) => {
            post_archive_run.audit_events.push(ManifestAuditEvent {
                event: "archiveCleanupPending".to_string(),
                at: unix_now(),
                phase_id: Some(reservation.phase_id.clone()),
                worker_thread_id: Some(reservation.worker_thread_id.clone()),
                detail: Some(error.to_string()),
            });
            "pending"
        }
    };
    if let Some(phase) = post_archive_run
        .phases
        .iter_mut()
        .find(|phase| phase.phase_id == reservation.phase_id)
    {
        phase.archive_cleanup_state = cleanup_state.to_string();
    }
    if reservation.next_phase_id.is_none() {
        post_archive_run.status = "completed".to_string();
        post_archive_run.audit_events.push(ManifestAuditEvent {
            event: "completed".to_string(),
            at: unix_now(),
            phase_id: None,
            worker_thread_id: None,
            detail: None,
        });
    }
    replace_manifest_run(&mut post_archive_state, &reservation.project_key, post_archive_run.clone())?;
    persist_state(runtime, &post_archive_state).await?;

    Ok(json!({
        "run": post_archive_run,
        "advancedPhaseId": reservation.phase_id,
        "archivedWorkerThreadId": reservation.worker_thread_id,
        "archiveCleanupState": cleanup_state,
        "nextWorkerThreadId": next_worker_thread_id,
    }))
}

pub async fn manifest_cancel(
    runtime: &BridgeRuntime,
    sender_thread_id: &str,
    run_id: &str,
    reason: Option<&str>,
) -> Result<Value> {
    let mut state = parse_state(&runtime.state_document_value().await);
    let project_key = project_key_for_thread(&state, sender_thread_id)
        .or_else(|| project_key_for_run(&state, run_id))
        .ok_or_else(|| anyhow::anyhow!("No tracked project found for manifest cancel."))?;
    let mut run = get_manifest_run(&state, &project_key, run_id)?;
    if matches!(run.status.as_str(), "completed" | "cancelled") {
        bail!("Manifest run `{run_id}` is already {}.", run.status);
    }
    run.status = "cancelled".to_string();
    run.current_phase_id = None;
    for phase in &mut run.phases {
        if phase.status == "ghost" {
            phase.status = "cancelled".to_string();
        }
        if phase.worker_thread_id.is_some() {
            phase.archive_safe = true;
            phase.archive_cleanup_state = "ready".to_string();
        }
    }
    for phase in &run.phases {
        if let Some(worker_thread_id) = &phase.worker_thread_id {
            mark_agent_manifest_archive_safe(&mut state, worker_thread_id);
        }
    }
    run.audit_events.push(ManifestAuditEvent {
        event: "cancelled".to_string(),
        at: unix_now(),
        phase_id: None,
        worker_thread_id: None,
        detail: reason.map(str::to_string),
    });
    run.updated_at = unix_now();
    replace_manifest_run(&mut state, &project_key, run.clone())?;
    persist_state(runtime, &state).await?;
    Ok(json!({ "run": run }))
}

pub(crate) fn manifest_archive_denial_for_agent(agent: &PersistedAgentState) -> Option<String> {
    let binding = agent
        .extras
        .get(MANIFEST_BINDING_KEY)
        .and_then(|value| serde_json::from_value::<ManifestBindingState>(value.clone()).ok())?;
    if binding.archive_safe {
        return None;
    }
    Some(format!(
        "Agent is bound to manifest `{}` phase `{}` and is not archive-safe yet. Use `robdex manifest advance` after Requirements review and handoff are complete, or `robdex manifest cancel` for the run.",
        binding.plan_id, binding.phase_id
    ))
}

pub(crate) fn manifest_runs_payload(state: &PersistedState, project_root: &str) -> Value {
    project_key_by_root(state, project_root)
        .and_then(|key| manifest_runs_for_project(state, &key).ok())
        .map(|runs| json!(runs))
        .unwrap_or_else(|| json!([]))
}

#[derive(Debug)]
struct AdvanceReservation {
    project_key: String,
    phase_id: String,
    phase_index: usize,
    worker_thread_id: String,
    next_phase_id: Option<String>,
}

fn reserve_manifest_advance_transition(
    state: &mut PersistedState,
    project_key: &str,
    run_id: &str,
    handoff: &str,
    handoff_path: String,
) -> Result<AdvanceReservation> {
    let mut run = get_manifest_run(state, project_key, run_id)?;
    if run.status != "active" {
        bail!("Manifest run `{run_id}` is not active.");
    }
    let current_phase_id = run
        .current_phase_id
        .clone()
        .ok_or_else(|| anyhow::anyhow!("Manifest run `{run_id}` has no current phase."))?;
    let phase_index = run
        .phases
        .iter()
        .position(|phase| phase.phase_id == current_phase_id)
        .ok_or_else(|| anyhow::anyhow!("Current phase `{current_phase_id}` is missing."))?;
    if run.phases[phase_index].status != "running" {
        bail!(
            "Manifest phase `{current_phase_id}` is `{}` and cannot be advanced; expected running.",
            run.phases[phase_index].status
        );
    }
    let worker_thread_id = run.phases[phase_index]
        .worker_thread_id
        .clone()
        .ok_or_else(|| anyhow::anyhow!("Current phase `{current_phase_id}` has no materialized worker."))?;
    let worker = agent_for_thread(state, &worker_thread_id)
        .ok_or_else(|| anyhow::anyhow!("Manifest worker `{worker_thread_id}` is not tracked."))?;
    ensure_requirements_passed(worker, &worker_thread_id)?;
    let latest_claim = worker
        .requirement_review
        .as_ref()
        .and_then(|review| review.latest_claim_packet.clone())
        .unwrap_or(Value::Null);
    let latest_verdict = worker
        .requirement_review
        .as_ref()
        .and_then(|review| review.latest_verdict_packet.clone())
        .unwrap_or(Value::Null);
    run.phases[phase_index].handoff = Some(ManifestHandoffState {
        text: handoff.to_string(),
        captured_at: unix_now(),
        source_path: Some(handoff_path),
    });
    run.phases[phase_index].durable_evidence = Some(json!({
        "latestClaimPacket": latest_claim,
        "latestVerdictPacket": latest_verdict,
        "copiedAt": unix_now(),
    }));
    run.phases[phase_index].status = "passed".to_string();
    run.phases[phase_index].passed_at = Some(unix_now());
    run.phases[phase_index].archive_safe = true;
    run.phases[phase_index].archive_cleanup_state = "ready".to_string();
    mark_agent_manifest_archive_safe(state, &worker_thread_id);
    run.audit_events.push(ManifestAuditEvent {
        event: "advanceReserved".to_string(),
        at: unix_now(),
        phase_id: Some(current_phase_id.clone()),
        worker_thread_id: Some(worker_thread_id.clone()),
        detail: Some("Requirements verdict and handoff copied before external materialization/archive.".to_string()),
    });
    let next_index = phase_index + 1;
    let next_phase_id = if next_index < run.phases.len() {
        let next_phase_id = run.phases[next_index].phase_id.clone();
        if run.phases[next_index].status != "ghost" {
            bail!(
                "Next manifest phase `{next_phase_id}` is `{}` and cannot be materialized; expected ghost.",
                run.phases[next_index].status
            );
        }
        run.current_phase_id = Some(next_phase_id.clone());
        run.phases[next_index].status = "materializing".to_string();
        run.audit_events.push(ManifestAuditEvent {
            event: "materializationReserved".to_string(),
            at: unix_now(),
            phase_id: Some(next_phase_id.clone()),
            worker_thread_id: None,
            detail: None,
        });
        Some(next_phase_id)
    } else {
        run.current_phase_id = None;
        run.status = "completing".to_string();
        None
    };
    run.updated_at = unix_now();
    replace_manifest_run(state, project_key, run)?;
    Ok(AdvanceReservation {
        project_key: project_key.to_string(),
        phase_id: current_phase_id,
        phase_index,
        worker_thread_id,
        next_phase_id,
    })
}

async fn materialize_current_phase(
    runtime: &BridgeRuntime,
    sender_thread_id: &str,
    state: &mut PersistedState,
    project_key: &str,
    run: &mut ManifestRunState,
    previous_phase_index: Option<usize>,
) -> Result<(String, String)> {
        let phase_id = run
            .current_phase_id
            .clone()
            .ok_or_else(|| anyhow::anyhow!("Manifest run has no current phase to materialize."))?;
        let phase_index = run
            .phases
            .iter()
            .position(|phase| phase.phase_id == phase_id)
            .ok_or_else(|| anyhow::anyhow!("Manifest phase `{phase_id}` is missing."))?;
        if run.phases[phase_index].worker_thread_id.is_some() {
            bail!("Manifest phase `{phase_id}` is already materialized.");
        }
        if run.phases[phase_index].status != "ghost" {
            bail!("Manifest phase `{phase_id}` is not a ghost phase.");
        }
        let handoff = previous_phase_index.and_then(|index| {
            run.phases
                .get(index)
                .and_then(|phase| phase.handoff.as_ref())
                .map(|handoff| handoff.text.clone())
        });
        let phase = run.phases[phase_index].clone();
        let requirement_set = RequirementSetState {
            id: Some(format!("{}-{}", run.plan_id, phase.phase_id)),
            active: true,
            enforce_on_turns: true,
            reviewer_thread_id: None,
            requirements: phase.requirements.clone(),
            review_progress: BTreeMap::new(),
        };
        let prompt = phase_prompt(run, &phase, handoff.as_deref());
        let spawn_payload = orchestrator_spawn_agent(
            runtime,
            sender_thread_id,
            &format!("{} / {}", run.title, phase.title),
            &prompt,
            None,
            Some("worker"),
            None,
            Some(serde_json::to_value(&requirement_set)?),
        )
        .await
        .context("failed to materialize manifest phase worker")?;
        let worker_thread_id = spawn_payload
            .get("agent")
            .and_then(Value::as_object)
            .and_then(|agent| {
                agent.get("threadID")
                    .or_else(|| agent.get("threadId"))
                    .or_else(|| agent.get("id"))
            })
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| anyhow::anyhow!("Materialization uncertainty: spawn response did not include worker thread id."))?;
        *state = parse_state(&runtime.state_document_value().await);
        bind_agent_to_manifest(
            state,
            &worker_thread_id,
            ManifestBindingState {
                run_id: run.run_id.clone(),
                plan_id: run.plan_id.clone(),
                phase_id: phase.phase_id.clone(),
                archive_safe: false,
            },
        )?;
        run.phases[phase_index].worker_thread_id = Some(worker_thread_id.clone());
        run.phases[phase_index].status = "running".to_string();
        run.phases[phase_index].archive_cleanup_state = "notReady".to_string();
        run.phases[phase_index].materialized_at = Some(unix_now());
        run.audit_events.push(ManifestAuditEvent {
            event: "materialized".to_string(),
            at: unix_now(),
            phase_id: Some(phase.phase_id.clone()),
            worker_thread_id: Some(worker_thread_id.clone()),
            detail: None,
        });
        if !state.projects.contains_key(project_key) {
            bail!("Project disappeared during manifest materialization.");
        }
        Ok((phase_id, worker_thread_id))
}

async fn materialize_reserved_phase(
    runtime: &BridgeRuntime,
    sender_thread_id: &str,
    state: &mut PersistedState,
    project_key: &str,
    run: &mut ManifestRunState,
    phase_id: &str,
    previous_phase_index: usize,
    completed_worker_thread_id: &str,
) -> Result<String> {
    let phase_index = run
        .phases
        .iter()
        .position(|phase| phase.phase_id == phase_id)
        .ok_or_else(|| anyhow::anyhow!("Manifest phase `{phase_id}` is missing."))?;
    if run.phases[phase_index].worker_thread_id.is_some() {
        bail!("Manifest phase `{phase_id}` is already materialized.");
    }
    if run.phases[phase_index].status != "materializing" {
        bail!(
            "Manifest phase `{phase_id}` is `{}` and cannot be materialized; expected materializing.",
            run.phases[phase_index].status
        );
    }
    let handoff = run
        .phases
        .get(previous_phase_index)
        .and_then(|phase| phase.handoff.as_ref())
        .map(|handoff| handoff.text.clone());
    let phase = run.phases[phase_index].clone();
    let requirement_set = RequirementSetState {
        id: Some(format!("{}-{}", run.plan_id, phase.phase_id)),
        active: true,
        enforce_on_turns: true,
        reviewer_thread_id: None,
        requirements: phase.requirements.clone(),
        review_progress: BTreeMap::new(),
    };
    let prompt = phase_prompt(run, &phase, handoff.as_deref());
    let spawn_payload = orchestrator_spawn_agent(
        runtime,
        sender_thread_id,
        &format!("{} / {}", run.title, phase.title),
        &prompt,
        None,
        Some("worker"),
        None,
        Some(serde_json::to_value(&requirement_set)?),
    )
    .await
    .context("failed to materialize manifest phase worker")?;
    let worker_thread_id = spawn_payload
        .get("agent")
        .and_then(Value::as_object)
        .and_then(|agent| {
            agent
                .get("threadID")
                .or_else(|| agent.get("threadId"))
                .or_else(|| agent.get("id"))
        })
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| anyhow::anyhow!("Materialization uncertainty: spawn response did not include worker thread id."))?;
    let _guard = runtime.lock_state_mutation().await;
    *state = parse_state(&runtime.state_document_value().await);
    *run = get_manifest_run(state, project_key, &run.run_id)?;
    let phase_index = run
        .phases
        .iter()
        .position(|phase| phase.phase_id == phase_id)
        .ok_or_else(|| anyhow::anyhow!("Manifest phase `{phase_id}` is missing after spawn."))?;
    if run.phases[phase_index].worker_thread_id.is_some() {
        bail!("Manifest phase `{phase_id}` is already materialized after spawn.");
    }
    if run.phases[phase_index].status != "materializing" {
        bail!(
            "Manifest phase `{phase_id}` is `{}` after spawn and cannot be bound; expected materializing.",
            run.phases[phase_index].status
        );
    }
    bind_agent_to_manifest(
        state,
        &worker_thread_id,
        ManifestBindingState {
            run_id: run.run_id.clone(),
            plan_id: run.plan_id.clone(),
            phase_id: phase.phase_id.clone(),
            archive_safe: false,
        },
    )?;
    mark_agent_manifest_archive_safe(state, completed_worker_thread_id);
    run.phases[phase_index].worker_thread_id = Some(worker_thread_id.clone());
    run.phases[phase_index].status = "running".to_string();
    run.phases[phase_index].archive_cleanup_state = "notReady".to_string();
    run.phases[phase_index].materialized_at = Some(unix_now());
    run.audit_events.push(ManifestAuditEvent {
        event: "materialized".to_string(),
        at: unix_now(),
        phase_id: Some(phase.phase_id.clone()),
        worker_thread_id: Some(worker_thread_id.clone()),
        detail: None,
    });
    if !state.projects.contains_key(project_key) {
        bail!("Project disappeared during manifest materialization.");
    }
    replace_manifest_run(state, project_key, run.clone())?;
    persist_state(runtime, state).await?;
    Ok(worker_thread_id)
}

fn phase_prompt(run: &ManifestRunState, phase: &ManifestPhaseRunState, handoff: Option<&str>) -> String {
    let mut prompt = format!(
        "You are materialized from serial manifest `{}` phase `{}`.\n\nManifest title: {}\nPhase title: {}\n\nComplete this phase exactly as specified. Future phases are not yours unless separately materialized.\n",
        run.plan_id, phase.phase_id, run.title, phase.title
    );
    if !phase.prompt.trim().is_empty() {
        prompt.push_str("\nPhase instructions:\n");
        prompt.push_str(phase.prompt.trim());
        prompt.push('\n');
    }
    if let Some(handoff) = handoff.map(str::trim).filter(|value| !value.is_empty()) {
        prompt.push_str("\nPrior phase handoff:\n");
        prompt.push_str(handoff);
        prompt.push('\n');
    }
    prompt
}

struct LoadedManifest {
    source_path: PathBuf,
    source_hash: String,
    source_snapshot: String,
    rationale_markdown: String,
    spec: ManifestSpec,
}

fn load_manifest_file(file_path: &str) -> Result<LoadedManifest> {
    let source_path = Path::new(file_path)
        .expanduser()
        .canonicalize()
        .with_context(|| format!("manifest file `{file_path}` is not readable"))?;
    if source_path.extension().and_then(|value| value.to_str()) != Some("md") {
        bail!("Manifest file must be Markdown with .md extension.");
    }
    let project_path = project_path_for_manifest(&source_path)?;
    let manifests_dir = project_path.join(".codex").join("manifests");
    if !source_path.starts_with(&manifests_dir) {
        bail!(
            "Manifest files must live under `{}`.",
            manifests_dir.display()
        );
    }
    let source_snapshot = fs::read_to_string(&source_path)
        .with_context(|| format!("failed to read {}", source_path.display()))?;
    let source_hash = format!("sha256:{:x}", Sha256::digest(source_snapshot.as_bytes()));
    let (frontmatter, rationale_markdown) = split_frontmatter(&source_snapshot)?;
    let rationale_markdown = rationale_markdown.to_string();
    let spec: ManifestSpec = serde_yaml::from_str(frontmatter)
        .with_context(|| format!("invalid manifest YAML in {}", source_path.display()))?;
    validate_manifest_spec(&spec)?;
    Ok(LoadedManifest {
        source_path,
        source_hash,
        source_snapshot,
        rationale_markdown,
        spec,
    })
}

fn split_frontmatter(text: &str) -> Result<(&str, &str)> {
    let Some(rest) = text.strip_prefix("---\n") else {
        bail!("Manifest must start with strict YAML frontmatter delimiter `---`.");
    };
    let closing = rest
        .find("\n---\n")
        .ok_or_else(|| anyhow::anyhow!("Manifest YAML frontmatter must close with `---`."))?;
    let yaml = &rest[..closing];
    let body = &rest[closing + 5..];
    Ok((yaml, body))
}

fn validate_manifest_spec(spec: &ManifestSpec) -> Result<()> {
    if spec.schema_version != 1 {
        bail!("Unsupported manifest schemaVersion `{}`; expected 1.", spec.schema_version);
    }
    if spec.plan_id.trim().is_empty() {
        bail!("Manifest planId is required.");
    }
    if spec.title.trim().is_empty() {
        bail!("Manifest title is required.");
    }
    if spec.phases.is_empty() {
        bail!("Manifest must define at least one phase.");
    }
    let mut ids = BTreeSet::new();
    for phase in &spec.phases {
        if phase.id.trim().is_empty() {
            bail!("Manifest phase id is required.");
        }
        if !ids.insert(phase.id.clone()) {
            bail!("Duplicate manifest phase id `{}`.", phase.id);
        }
        if phase.title.trim().is_empty() {
            bail!("Manifest phase `{}` title is required.", phase.id);
        }
        if phase.requirements.is_empty() {
            bail!("Manifest phase `{}` must define Requirements.", phase.id);
        }
        let mut requirement_keys = BTreeSet::new();
        for requirement in &phase.requirements {
            if requirement.key.trim().is_empty() || requirement.statement.trim().is_empty() {
                bail!("Manifest phase `{}` has an invalid Requirement.", phase.id);
            }
            if !requirement_keys.insert(requirement.key.clone()) {
                bail!("Manifest phase `{}` has duplicate Requirement key `{}`.", phase.id, requirement.key);
            }
        }
    }
    Ok(())
}

fn project_path_for_manifest(source_path: &Path) -> Result<PathBuf> {
    let components = source_path.components().collect::<Vec<_>>();
    for index in 0..components.len().saturating_sub(2) {
        let maybe_codex = components[index].as_os_str().to_string_lossy();
        let maybe_manifests = components[index + 1].as_os_str().to_string_lossy();
        if maybe_codex == ".codex" && maybe_manifests == "manifests" {
            let mut root = PathBuf::new();
            for component in &components[..index] {
                root.push(component.as_os_str());
            }
            if root.as_os_str().is_empty() {
                break;
            }
            return Ok(root);
        }
    }
    bail!("Manifest path must be inside PROJECT/.codex/manifests/.");
}

trait ExpandUser {
    fn expanduser(&self) -> PathBuf;
}

impl ExpandUser for Path {
    fn expanduser(&self) -> PathBuf {
        let text = self.display().to_string();
        if let Some(stripped) = text.strip_prefix("~/") {
            if let Some(home) = std::env::var_os("HOME") {
                return PathBuf::from(home).join(stripped);
            }
        }
        self.to_path_buf()
    }
}

fn project_key_by_root(state: &PersistedState, project_root: &str) -> Option<String> {
    let target = normalize_path_text(project_root);
    state.projects.iter().find_map(|(key, project)| {
        let root = project.project_root.as_deref().or(project.cwd.as_deref())?;
        (normalize_path_text(root) == target).then(|| key.clone())
    })
}

fn project_key_for_thread(state: &PersistedState, thread_id: &str) -> Option<String> {
    state.projects.iter().find_map(|(key, project)| {
        (project.agents.contains_key(thread_id) || project.orchestrator_thread_id.as_deref() == Some(thread_id))
            .then(|| key.clone())
    })
}

fn project_key_for_run(state: &PersistedState, run_id: &str) -> Option<String> {
    state.projects.keys().find_map(|key| {
        manifest_runs_for_project(state, key)
            .ok()
            .and_then(|runs| runs.into_iter().any(|run| run.run_id == run_id).then(|| key.clone()))
    })
}

fn normalize_path_text(value: &str) -> String {
    Path::new(value)
        .expanduser()
        .canonicalize()
        .unwrap_or_else(|_| Path::new(value).expanduser())
        .display()
        .to_string()
}

fn manifest_runs_for_project(state: &PersistedState, project_key: &str) -> Result<Vec<ManifestRunState>> {
    let Some(project) = state.projects.get(project_key) else {
        return Ok(Vec::new());
    };
    let Some(value) = project.extras.get(PROJECT_MANIFEST_RUNS_KEY) else {
        return Ok(Vec::new());
    };
    serde_json::from_value(value.clone()).context("invalid persisted manifest runs")
}

fn insert_manifest_run(state: &mut PersistedState, project_key: &str, run: ManifestRunState) -> Result<()> {
    let mut runs = manifest_runs_for_project(state, project_key)?;
    runs.push(run);
    let project = state
        .projects
        .get_mut(project_key)
        .ok_or_else(|| anyhow::anyhow!("Project `{project_key}` is missing."))?;
    project
        .extras
        .insert(PROJECT_MANIFEST_RUNS_KEY.to_string(), json!(runs));
    Ok(())
}

fn replace_manifest_run(state: &mut PersistedState, project_key: &str, run: ManifestRunState) -> Result<()> {
    let mut runs = manifest_runs_for_project(state, project_key)?;
    let Some(existing) = runs.iter_mut().find(|existing| existing.run_id == run.run_id) else {
        bail!("Manifest run `{}` is missing.", run.run_id);
    };
    *existing = run;
    let project = state
        .projects
        .get_mut(project_key)
        .ok_or_else(|| anyhow::anyhow!("Project `{project_key}` is missing."))?;
    project
        .extras
        .insert(PROJECT_MANIFEST_RUNS_KEY.to_string(), json!(runs));
    Ok(())
}

fn get_manifest_run(state: &PersistedState, project_key: &str, run_id: &str) -> Result<ManifestRunState> {
    manifest_runs_for_project(state, project_key)?
        .into_iter()
        .find(|run| run.run_id == run_id)
        .ok_or_else(|| anyhow::anyhow!("Manifest run `{run_id}` was not found."))
}

fn ensure_no_active_conflict(
    state: &PersistedState,
    project_key: &str,
    plan_id: &str,
    source_hash: &str,
) -> Result<()> {
    for run in manifest_runs_for_project(state, project_key)? {
        if run.plan_id == plan_id
            && run.source_hash == source_hash
            && matches!(run.status.as_str(), "active" | "blocked")
        {
            bail!(
                "Active manifest run conflict for project `{}`, plan `{plan_id}`, source hash `{source_hash}`.",
                run.project_path
            );
        }
    }
    Ok(())
}

fn bind_agent_to_manifest(
    state: &mut PersistedState,
    worker_thread_id: &str,
    binding: ManifestBindingState,
) -> Result<()> {
    for project in state.projects.values_mut() {
        if let Some(agent) = project.agents.get_mut(worker_thread_id) {
            agent
                .extras
                .insert(MANIFEST_BINDING_KEY.to_string(), json!(binding));
            return Ok(());
        }
    }
    bail!("Manifest materialized worker `{worker_thread_id}` is not tracked.")
}

fn mark_agent_manifest_archive_safe(state: &mut PersistedState, worker_thread_id: &str) {
    for project in state.projects.values_mut() {
        if let Some(agent) = project.agents.get_mut(worker_thread_id) {
            if let Some(value) = agent.extras.get(MANIFEST_BINDING_KEY).cloned() {
                if let Ok(mut binding) = serde_json::from_value::<ManifestBindingState>(value) {
                    binding.archive_safe = true;
                    agent
                        .extras
                        .insert(MANIFEST_BINDING_KEY.to_string(), json!(binding));
                }
            }
        }
    }
}

fn agent_for_thread<'a>(state: &'a PersistedState, thread_id: &str) -> Option<&'a PersistedAgentState> {
    state.projects.values().find_map(|project| project.agents.get(thread_id))
}

fn ensure_requirements_passed(agent: &PersistedAgentState, worker_thread_id: &str) -> Result<()> {
    let Some(requirements) = agent.requirements.as_ref() else {
        bail!("Manifest worker `{worker_thread_id}` has no Requirements.");
    };
    let Some(review) = agent.requirement_review.as_ref() else {
        bail!("Manifest worker `{worker_thread_id}` has no Requirements review verdict.");
    };
    if review.status != "passed" {
        bail!(
            "Manifest worker `{worker_thread_id}` Requirements review is `{}`; expected passed.",
            review.status
        );
    }
    for requirement in &requirements.requirements {
        if requirements
            .review_progress
            .get(requirement.key.as_str())
            .map(|progress| progress.status.as_str())
            != Some("passed")
        {
            bail!(
                "Manifest worker `{worker_thread_id}` Requirement `{}` is not passed.",
                requirement.key
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn requirement(key: &str) -> RequirementState {
        RequirementState {
            key: key.to_string(),
            statement: format!("{key} statement"),
            severity: "blocker".to_string(),
            claim_schema_description: None,
            verdict_schema_description: None,
            verification_method: "manualEvidence".to_string(),
        }
    }

    fn two_phase_run() -> ManifestRunState {
        ManifestRunState {
            run_id: "run".to_string(),
            plan_id: "plan".to_string(),
            title: "Plan".to_string(),
            schema_version: 1,
            project_path: "/tmp/project".to_string(),
            source_path: "/tmp/project/.codex/manifests/plan.md".to_string(),
            source_hash: "sha256:abc".to_string(),
            source_snapshot: String::new(),
            rationale_markdown: String::new(),
            status: "active".to_string(),
            current_phase_id: Some("phase-1".to_string()),
            phases: vec![
                ManifestPhaseRunState {
                    phase_id: "phase-1".to_string(),
                    title: "Phase 1".to_string(),
                    status: "running".to_string(),
                    prompt: String::new(),
                    requirements: vec![requirement("phaseOneDone")],
                    worker_thread_id: Some("worker-1".to_string()),
                    handoff: None,
                    durable_evidence: None,
                    blocker: None,
                    waiver: None,
                    resume_decision: None,
                    archive_cleanup_state: "notReady".to_string(),
                    archive_safe: false,
                    materialized_at: Some(1),
                    passed_at: None,
                },
                ManifestPhaseRunState {
                    phase_id: "phase-2".to_string(),
                    title: "Phase 2".to_string(),
                    status: "ghost".to_string(),
                    prompt: String::new(),
                    requirements: vec![requirement("phaseTwoDone")],
                    worker_thread_id: None,
                    handoff: None,
                    durable_evidence: None,
                    blocker: None,
                    waiver: None,
                    resume_decision: None,
                    archive_cleanup_state: "notReady".to_string(),
                    archive_safe: false,
                    materialized_at: None,
                    passed_at: None,
                },
            ],
            audit_events: Vec::new(),
            activated_at: 1,
            updated_at: 1,
        }
    }

    fn state_with_manifest_worker(run: ManifestRunState, review_status: &str) -> PersistedState {
        let mut state = PersistedState::default();
        let mut project = crate::commands::PersistedProjectState {
            id: Some("project".to_string()),
            name: Some("Project".to_string()),
            project_root: Some("/tmp/project".to_string()),
            ..Default::default()
        };
        let mut progress = BTreeMap::new();
        progress.insert(
            "phaseOneDone".to_string(),
            crate::commands::RequirementReviewProgressState {
                status: "passed".to_string(),
                updated_at: Some(1),
            },
        );
        let mut agent = PersistedAgentState {
            role: Some("worker".to_string()),
            requirements: Some(RequirementSetState {
                id: Some("plan-phase-1".to_string()),
                active: false,
                enforce_on_turns: false,
                reviewer_thread_id: Some("reviewer-1".to_string()),
                requirements: vec![requirement("phaseOneDone")],
                review_progress: progress,
            }),
            requirement_review: Some(crate::commands::RequirementReviewBindingState {
                source_thread_id: "worker-1".to_string(),
                status: review_status.to_string(),
                reviewer_thread_id: "reviewer-1".to_string(),
                requirement_set_id: Some("plan-phase-1".to_string()),
                latest_claim_packet: Some(json!({"requirements":{"phaseOneDone":{"claim":"satisfied"}}})),
                latest_verdict_packet: Some(json!({"requirements":{"phaseOneDone":{"verdict":"pass"}}})),
                updated_at: 1,
            }),
            ..Default::default()
        };
        agent.extras.insert(
            MANIFEST_BINDING_KEY.to_string(),
            json!(ManifestBindingState {
                run_id: "run".to_string(),
                plan_id: "plan".to_string(),
                phase_id: "phase-1".to_string(),
                archive_safe: false,
            }),
        );
        project.agents.insert("worker-1".to_string(), agent);
        project
            .extras
            .insert(PROJECT_MANIFEST_RUNS_KEY.to_string(), json!(vec![run]));
        state.projects.insert("project".to_string(), project);
        state
    }

    #[test]
    fn strict_frontmatter_parser_rejects_missing_delimiter() {
        let error = split_frontmatter("planId: nope\n---\nbody").expect_err("missing delimiter");
        assert!(error.to_string().contains("must start"));
    }

    #[test]
    fn strict_manifest_yaml_rejects_unknown_fields() {
        let yaml = r#"
schemaVersion: 1
planId: plan
title: Plan
extra: nope
phases:
  - id: phase
    title: Phase
    requirements:
      - key: done
        statement: Do it.
"#;
        let error = serde_yaml::from_str::<ManifestSpec>(yaml).expect_err("unknown field");
        assert!(error.to_string().contains("unknown field"));
    }

    #[test]
    fn manifest_validation_requires_phase_requirements() {
        let spec = ManifestSpec {
            schema_version: 1,
            plan_id: "plan".to_string(),
            title: "Plan".to_string(),
            phases: vec![ManifestPhaseSpec {
                id: "phase".to_string(),
                title: "Phase".to_string(),
                prompt: String::new(),
                requirements: Vec::new(),
            }],
        };
        let error = validate_manifest_spec(&spec).expect_err("requirements required");
        assert!(error.to_string().contains("must define Requirements"));
    }

    #[test]
    fn active_run_uniqueness_uses_project_plan_and_source_hash() {
        let mut state = PersistedState::default();
        state.projects.insert(
            "project".to_string(),
            crate::commands::PersistedProjectState {
                id: Some("project".to_string()),
                name: Some("Project".to_string()),
                project_root: Some("/tmp/project".to_string()),
                ..Default::default()
            },
        );
        insert_manifest_run(
            &mut state,
            "project",
            ManifestRunState {
                run_id: "run".to_string(),
                plan_id: "plan".to_string(),
                title: "Plan".to_string(),
                schema_version: 1,
                project_path: "/tmp/project".to_string(),
                source_path: "/tmp/project/.codex/manifests/plan.md".to_string(),
                source_hash: "sha256:abc".to_string(),
                source_snapshot: String::new(),
                rationale_markdown: String::new(),
                status: "active".to_string(),
                current_phase_id: Some("phase".to_string()),
                phases: Vec::new(),
                audit_events: Vec::new(),
                activated_at: 1,
                updated_at: 1,
            },
        )
        .expect("insert");
        assert!(ensure_no_active_conflict(&state, "project", "plan", "sha256:abc").is_err());
        assert!(ensure_no_active_conflict(&state, "project", "plan", "sha256:def").is_ok());
    }

    #[test]
    fn manifest_archive_denial_mentions_lifecycle_command() {
        let mut agent = PersistedAgentState::default();
        agent.extras.insert(
            MANIFEST_BINDING_KEY.to_string(),
            json!(ManifestBindingState {
                run_id: "run".to_string(),
                plan_id: "plan".to_string(),
                phase_id: "phase".to_string(),
                archive_safe: false,
            }),
        );
        let denial = manifest_archive_denial_for_agent(&agent).expect("denial");
        assert!(denial.contains("manifest `plan` phase `phase`"));
        assert!(denial.contains("robdex manifest advance"));
    }

    #[test]
    fn manifest_archive_denial_clears_after_archive_safe_mark() {
        let mut state = PersistedState::default();
        let mut agent = PersistedAgentState {
            role: Some("worker".to_string()),
            ..Default::default()
        };
        agent.extras.insert(
            MANIFEST_BINDING_KEY.to_string(),
            json!(ManifestBindingState {
                run_id: "run".to_string(),
                plan_id: "plan".to_string(),
                phase_id: "phase".to_string(),
                archive_safe: false,
            }),
        );
        let mut project = crate::commands::PersistedProjectState::default();
        project.agents.insert("worker-1".to_string(), agent);
        state.projects.insert("project".to_string(), project);

        let worker = state.projects["project"].agents.get("worker-1").expect("worker");
        assert!(manifest_archive_denial_for_agent(worker).is_some());

        mark_agent_manifest_archive_safe(&mut state, "worker-1");
        let worker = state.projects["project"].agents.get("worker-1").expect("worker");
        assert!(manifest_archive_denial_for_agent(worker).is_none());
    }

    #[test]
    fn manifest_phase_decision_fields_survive_json_persistence() {
        let run = ManifestRunState {
            run_id: "run".to_string(),
            plan_id: "plan".to_string(),
            title: "Plan".to_string(),
            schema_version: 1,
            project_path: "/tmp/project".to_string(),
            source_path: "/tmp/project/.codex/manifests/plan.md".to_string(),
            source_hash: "sha256:abc".to_string(),
            source_snapshot: String::new(),
            rationale_markdown: String::new(),
            status: "active".to_string(),
            current_phase_id: Some("phase".to_string()),
            phases: vec![ManifestPhaseRunState {
                phase_id: "phase".to_string(),
                title: "Phase".to_string(),
                status: "running".to_string(),
                prompt: String::new(),
                requirements: Vec::new(),
                worker_thread_id: Some("worker-1".to_string()),
                handoff: None,
                durable_evidence: None,
                blocker: Some(json!({"text": "blocked"})),
                waiver: Some(json!({"text": "waived"})),
                resume_decision: Some(json!({"text": "resume"})),
                archive_cleanup_state: "notReady".to_string(),
                archive_safe: false,
                materialized_at: Some(1),
                passed_at: None,
            }],
            audit_events: vec![ManifestAuditEvent {
                event: "blockerRecorded".to_string(),
                at: 1,
                phase_id: Some("phase".to_string()),
                worker_thread_id: Some("worker-1".to_string()),
                detail: Some("blocked".to_string()),
            }],
            activated_at: 1,
            updated_at: 1,
        };
        let encoded = serde_json::to_value(&run).expect("encode");
        let decoded = serde_json::from_value::<ManifestRunState>(encoded).expect("decode");
        let phase = &decoded.phases[0];
        assert!(phase.blocker.is_some());
        assert!(phase.waiver.is_some());
        assert!(phase.resume_decision.is_some());
        assert_eq!(decoded.audit_events[0].event, "blockerRecorded");
    }

    #[test]
    fn manifest_advance_reservation_is_atomic_and_prevents_duplicate_materialization() {
        let mut state = state_with_manifest_worker(two_phase_run(), "passed");
        let reservation = reserve_manifest_advance_transition(
            &mut state,
            "project",
            "run",
            "handoff",
            "/tmp/handoff.md".to_string(),
        )
        .expect("reserve");
        assert_eq!(reservation.phase_id, "phase-1");
        assert_eq!(reservation.next_phase_id.as_deref(), Some("phase-2"));

        let run = get_manifest_run(&state, "project", "run").expect("run");
        assert_eq!(run.current_phase_id.as_deref(), Some("phase-2"));
        assert_eq!(run.phases[0].status, "passed");
        assert_eq!(run.phases[0].archive_cleanup_state, "ready");
        assert!(run.phases[0].archive_safe);
        assert!(run.phases[0].handoff.is_some());
        assert!(run.phases[0].durable_evidence.is_some());
        assert_eq!(run.phases[1].status, "materializing");
        assert!(run.phases[1].worker_thread_id.is_none());
        assert!(run.audit_events.iter().any(|event| event.event == "advanceReserved"));
        assert!(run.audit_events.iter().any(|event| event.event == "materializationReserved"));

        let worker = state.projects["project"].agents.get("worker-1").expect("worker");
        assert!(manifest_archive_denial_for_agent(worker).is_none());

        let duplicate = reserve_manifest_advance_transition(
            &mut state,
            "project",
            "run",
            "handoff again",
            "/tmp/handoff2.md".to_string(),
        )
        .expect_err("duplicate reserve must fail");
        assert!(duplicate.to_string().contains("expected running"));

        let run = get_manifest_run(&state, "project", "run").expect("run");
        assert_eq!(run.phases[1].status, "materializing");
        assert!(run.phases[1].worker_thread_id.is_none());
    }

    #[test]
    fn manifest_advance_reservation_requires_passed_requirements_review() {
        let mut state = state_with_manifest_worker(two_phase_run(), "failed");
        let error = reserve_manifest_advance_transition(
            &mut state,
            "project",
            "run",
            "handoff",
            "/tmp/handoff.md".to_string(),
        )
        .expect_err("failed review blocks advance");
        assert!(error.to_string().contains("Requirements review is `failed`; expected passed"));

        let run = get_manifest_run(&state, "project", "run").expect("run");
        assert_eq!(run.current_phase_id.as_deref(), Some("phase-1"));
        assert_eq!(run.phases[0].status, "running");
        assert_eq!(run.phases[1].status, "ghost");
    }

    #[test]
    fn manifest_advance_reservation_rejects_already_materializing_next_phase() {
        let mut run = two_phase_run();
        run.phases[1].status = "materializing".to_string();
        let mut state = state_with_manifest_worker(run, "passed");
        let error = reserve_manifest_advance_transition(
            &mut state,
            "project",
            "run",
            "handoff",
            "/tmp/handoff.md".to_string(),
        )
        .expect_err("non-ghost next phase blocks advance");
        assert!(error.to_string().contains("expected ghost"));

        let run = get_manifest_run(&state, "project", "run").expect("run");
        assert_eq!(run.current_phase_id.as_deref(), Some("phase-1"));
        assert_eq!(run.phases[0].status, "running");
        assert_eq!(run.phases[1].status, "materializing");
    }

    #[test]
    fn manifest_advance_reservation_marks_final_phase_completing_before_archive() {
        let mut run = two_phase_run();
        run.current_phase_id = Some("phase-2".to_string());
        run.phases[0].status = "passed".to_string();
        run.phases[0].archive_safe = true;
        run.phases[0].archive_cleanup_state = "archived".to_string();
        run.phases[1].status = "running".to_string();
        run.phases[1].worker_thread_id = Some("worker-1".to_string());
        run.phases[1].requirements = vec![requirement("phaseOneDone")];
        let mut state = state_with_manifest_worker(run, "passed");

        let reservation = reserve_manifest_advance_transition(
            &mut state,
            "project",
            "run",
            "final handoff",
            "/tmp/final-handoff.md".to_string(),
        )
        .expect("reserve final phase");
        assert_eq!(reservation.next_phase_id, None);

        let run = get_manifest_run(&state, "project", "run").expect("run");
        assert_eq!(run.status, "completing");
        assert_eq!(run.current_phase_id, None);
        assert_eq!(run.phases[1].status, "passed");
        assert_eq!(run.phases[1].archive_cleanup_state, "ready");
        assert!(run.phases[1].archive_safe);
    }
}

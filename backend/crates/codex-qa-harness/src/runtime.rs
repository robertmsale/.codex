use std::{collections::BTreeMap, path::PathBuf, sync::Arc, time::Duration};

use anyhow::{Result, anyhow, bail};
use tokio::sync::{Mutex, OwnedMutexGuard, RwLock};
use tracing::warn;

use crate::{
    config::{HarnessArgs, HarnessConfig, ProjectConfig, load_harness_config},
    events::{EventBus, HarnessEvent},
    hook_runner::{HookRunRequest, HookRunner},
    ios_sim::{SharedSimulatorController, SimulatorStatus, default_controller},
    models::{
        CommandRequest, DeviceSummary, ErrorRecord, LeaseRecord, LeaseRequest, ProcessRecord,
        ProjectSummary, SlotPhase, SlotRuntimeState, SlotStatus, StartRequest,
    },
    state::{StateStore, iso_timestamp},
};

pub type SharedHarnessRuntime = Arc<HarnessRuntime>;

pub struct HarnessRuntime {
    config: HarnessConfig,
    store: StateStore,
    hook_runner: HookRunner,
    sim_controller: SharedSimulatorController,
    events: EventBus,
    slots: RwLock<BTreeMap<(String, String), SlotRuntimeState>>,
    slot_guards: BTreeMap<(String, String), Arc<Mutex<()>>>,
}

impl HarnessRuntime {
    pub fn load(args: &HarnessArgs) -> Result<SharedHarnessRuntime> {
        let config = load_harness_config(&args.config_dir)?;
        Self::from_config(config, args.state_root.clone())
    }

    pub fn from_config(config: HarnessConfig, state_root: std::path::PathBuf) -> Result<SharedHarnessRuntime> {
        Self::from_parts(config, state_root, HookRunner, default_controller())
    }

    pub fn from_parts(
        config: HarnessConfig,
        state_root: PathBuf,
        hook_runner: HookRunner,
        sim_controller: SharedSimulatorController,
    ) -> Result<SharedHarnessRuntime> {
        let store = StateStore::new(state_root);
        store.ensure_layout()?;
        let mut states = store.load_slot_states(&config.projects)?;
        reconcile_slot_processes(&store, &mut states)?;
        let slot_guards = config
            .projects
            .iter()
            .flat_map(|(project_id, project)| {
                project.devices.keys().map(move |device_key| {
                    (
                        (project_id.clone(), device_key.clone()),
                        Arc::new(Mutex::new(())),
                    )
                })
            })
            .collect();
        Ok(Arc::new(Self {
            config,
            store,
            hook_runner,
            sim_controller,
            events: EventBus::default(),
            slots: RwLock::new(states),
            slot_guards,
        }))
    }

    pub fn project_summaries(&self) -> Vec<ProjectSummary> {
        self.config
            .projects
            .values()
            .map(|project| ProjectSummary {
                id: project.id.clone(),
                display_name: project.display_name.clone(),
                repo_root: project.repo_root.clone(),
                runtime_root: project.runtime_root.clone(),
                device_count: project.devices.len(),
            })
            .collect()
    }

    pub async fn list_devices(&self, project_id: &str) -> Result<Vec<DeviceSummary>> {
        let project = self.project(project_id)?;
        let slots = self.slots.read().await;
        let devices = project
            .devices
            .iter()
            .map(|(device_key, device)| DeviceSummary {
                project_id: project.id.clone(),
                device_key: device_key.clone(),
                device_id: device.device_id.clone(),
                name: device.name.clone(),
                device_type: device.device_type.clone(),
                runtime_dir: self.store.runtime_dir(project, &device.runtime_subdir),
                boot_policy: device.boot_policy.clone(),
                state: slots
                    .get(&(project_id.to_string(), device_key.clone()))
                    .cloned()
                    .expect("slot state missing for configured device"),
            })
            .collect();
        Ok(devices)
    }

    pub async fn device_summary(&self, project_id: &str, device_key: &str) -> Result<DeviceSummary> {
        let devices = self.list_devices(project_id).await?;
        devices
            .into_iter()
            .find(|device| device.device_key == device_key)
            .ok_or_else(|| anyhow!("unknown device {device_key} in project {project_id}"))
    }

    pub async fn acquire_lease(
        &self,
        project_id: &str,
        device_key: &str,
        request: LeaseRequest,
    ) -> Result<SlotRuntimeState> {
        let _slot_guard = self.lock_slot(project_id, device_key).await?;
        let _project = self.project(project_id)?;
        let mut slots = self.slots.write().await;
        let state = self.slot_mut(&mut slots, project_id, device_key)?;
        if let Some(existing) = &state.lease {
            if existing.owner != request.owner {
                bail!(
                    "device {device_key} in project {project_id} already leased by {}",
                    existing.owner
                );
            }
        }
        state.lease = Some(LeaseRecord {
            owner: request.owner,
            reason: request.reason,
            acquired_at: iso_timestamp(),
            expires_at: request.expires_at,
        });
        state.updated_at = iso_timestamp();
        self.store.persist_slot_state(state)?;
        let snapshot = state.clone();
        drop(slots);
        self.events.publish_slot_state("lease_acquired", &snapshot);
        Ok(snapshot)
    }

    pub async fn release_lease(&self, project_id: &str, device_key: &str) -> Result<SlotRuntimeState> {
        let _slot_guard = self.lock_slot(project_id, device_key).await?;
        let mut slots = self.slots.write().await;
        let state = self.slot_mut(&mut slots, project_id, device_key)?;
        state.lease = None;
        state.updated_at = iso_timestamp();
        self.store.persist_slot_state(state)?;
        let snapshot = state.clone();
        drop(slots);
        self.events.publish_slot_state("lease_released", &snapshot);
        Ok(snapshot)
    }

    pub async fn simulator_status(&self, project_id: &str, device_key: &str) -> Result<SimulatorStatus> {
        let summary = self.device_summary(project_id, device_key).await?;
        self.sim_controller.inspect(&summary.device_id)
    }

    pub async fn start(
        &self,
        project_id: &str,
        device_key: &str,
        request: StartRequest,
    ) -> Result<SlotRuntimeState> {
        let _slot_guard = self.lock_slot(project_id, device_key).await?;
        self.run_start_sequence(project_id, device_key, request, false).await
    }

    pub async fn restart(
        &self,
        project_id: &str,
        device_key: &str,
        request: StartRequest,
    ) -> Result<SlotRuntimeState> {
        let _slot_guard = self.lock_slot(project_id, device_key).await?;
        self.run_start_sequence(project_id, device_key, request, true).await
    }

    pub async fn teardown(
        &self,
        project_id: &str,
        device_key: &str,
        request: StartRequest,
    ) -> Result<SlotRuntimeState> {
        let _slot_guard = self.lock_slot(project_id, device_key).await?;
        let project = self.project(project_id)?.clone();
        self.run_hook_for_slot(
            project_id,
            device_key,
            &request.lease_owner,
            SlotStatus::Stopping,
            SlotPhase::Teardown,
            &project.hooks.teardown,
            project.timeouts.teardown_sec,
            serde_json::json!({
                "project_id": project_id,
                "device_key": device_key,
                "lease_owner": request.lease_owner,
                "startup": request.startup,
            }),
            Some(SlotStatus::Idle),
        )
        .await
    }

    pub async fn command(
        &self,
        project_id: &str,
        device_key: &str,
        request: CommandRequest,
    ) -> Result<SlotRuntimeState> {
        let _slot_guard = self.lock_slot(project_id, device_key).await?;
        let mut slots = self.slots.write().await;
        let state = self.slot_mut(&mut slots, project_id, device_key)?;
        ensure_lease_owner(state, &request.lease_owner)?;
        drop(slots);

        let project = self.project(project_id)?.clone();
        let device = project
            .devices
            .get(device_key)
            .ok_or_else(|| anyhow!("unknown device {device_key} in project {project_id}"))?;
        self.ensure_simulator_ready(project_id, device_key, &request.lease_owner, &device.device_id)
            .await?;
        self.run_hook_for_slot(
            project_id,
            device_key,
            &request.lease_owner,
            SlotStatus::Busy,
            SlotPhase::ExecuteCommand,
            &project.hooks.command,
            project.timeouts.command_sec,
            serde_json::json!({
                "project_id": project_id,
                "device_key": device_key,
                "lease_owner": request.lease_owner,
                "command": request.command,
                "args": request.args,
            }),
            Some(SlotStatus::Ready),
        )
        .await
    }

    pub fn project_count(&self) -> usize {
        self.config.projects.len()
    }

    pub fn state_root(&self) -> &std::path::Path {
        self.store.root()
    }

    pub fn subscribe_events(&self) -> tokio::sync::broadcast::Receiver<HarnessEvent> {
        self.events.subscribe()
    }

    fn project(&self, project_id: &str) -> Result<&ProjectConfig> {
        self.config
            .projects
            .get(project_id)
            .ok_or_else(|| anyhow!("unknown project {project_id}"))
    }

    fn slot_mut<'a>(
        &self,
        slots: &'a mut BTreeMap<(String, String), SlotRuntimeState>,
        project_id: &str,
        device_key: &str,
    ) -> Result<&'a mut SlotRuntimeState> {
        slots
            .get_mut(&(project_id.to_string(), device_key.to_string()))
            .ok_or_else(|| anyhow!("unknown device {device_key} in project {project_id}"))
    }

    async fn lock_slot(
        &self,
        project_id: &str,
        device_key: &str,
    ) -> Result<OwnedMutexGuard<()>> {
        let guard = self
            .slot_guards
            .get(&(project_id.to_string(), device_key.to_string()))
            .ok_or_else(|| anyhow!("unknown device {device_key} in project {project_id}"))?;
        Ok(guard.clone().lock_owned().await)
    }

    async fn run_start_sequence(
        &self,
        project_id: &str,
        device_key: &str,
        request: StartRequest,
        _is_restart: bool,
    ) -> Result<SlotRuntimeState> {
        let project = self.project(project_id)?.clone();
        let device = project
            .devices
            .get(device_key)
            .ok_or_else(|| anyhow!("unknown device {device_key} in project {project_id}"))?;
        self.ensure_simulator_ready(project_id, device_key, &request.lease_owner, &device.device_id)
            .await?;
        self.run_hook_for_slot(
            project_id,
            device_key,
            &request.lease_owner,
            SlotStatus::Preparing,
            SlotPhase::PrepareSource,
            &project.hooks.prepare_source,
            project.timeouts.prepare_source_sec,
            serde_json::json!({
                "project_id": project_id,
                "device_key": device_key,
                "lease_owner": request.lease_owner,
                "startup": request.startup,
            }),
            Some(SlotStatus::Preparing),
        )
        .await?;

        if let Some(start_dependencies) = &project.hooks.start_dependencies {
            self.run_hook_for_slot(
                project_id,
                device_key,
                &request.lease_owner,
                SlotStatus::StartingDependencies,
                SlotPhase::StartDependencies,
                start_dependencies,
                project.timeouts.start_dependencies_sec,
                serde_json::json!({
                    "project_id": project_id,
                    "device_key": device_key,
                    "lease_owner": request.lease_owner,
                    "startup": request.startup,
                }),
                Some(SlotStatus::StartingRuntime),
            )
            .await?;
        }

        self.run_hook_for_slot(
            project_id,
            device_key,
            &request.lease_owner,
            SlotStatus::StartingRuntime,
            SlotPhase::StartRuntime,
            &project.hooks.start_runtime,
            project.timeouts.start_runtime_sec,
            serde_json::json!({
                "project_id": project_id,
                "device_key": device_key,
                "lease_owner": request.lease_owner,
                "startup": request.startup,
            }),
            Some(SlotStatus::StartingRuntime),
        )
        .await?;

        self.run_hook_for_slot(
            project_id,
            device_key,
            &request.lease_owner,
            SlotStatus::StartingRuntime,
            SlotPhase::CheckReadiness,
            &project.hooks.check_readiness,
            project.timeouts.readiness_sec,
            serde_json::json!({
                "project_id": project_id,
                "device_key": device_key,
                "lease_owner": request.lease_owner,
                "startup": request.startup,
            }),
            Some(SlotStatus::Ready),
        )
        .await
    }

    async fn ensure_simulator_ready(
        &self,
        project_id: &str,
        device_key: &str,
        lease_owner: &str,
        device_id: &str,
    ) -> Result<()> {
        let status = self.sim_controller.inspect(device_id)?;
        if matches!(status.booted, Some(true)) {
            return Ok(());
        }

        {
            let mut slots = self.slots.write().await;
            let state = self.slot_mut(&mut slots, project_id, device_key)?;
            ensure_lease_owner(state, lease_owner)?;
            state.status = SlotStatus::BootingSimulator;
            state.phase = SlotPhase::BootSimulator;
            state.last_error = None;
            state.updated_at = iso_timestamp();
            self.store.persist_slot_state(state)?;
            let snapshot = state.clone();
            drop(slots);
            self.events.publish_slot_state("phase_started", &snapshot);
        }

        self.sim_controller.boot(device_id).map_err(|error| {
            anyhow!("boot simulator {device_id} for {project_id}/{device_key}: {error}")
        })?;

        let verified = self.sim_controller.inspect(device_id)?;
        if !matches!(verified.booted, Some(true)) {
            let _ = self
                .apply_hook_failure(
                    project_id,
                    device_key,
                    SlotPhase::BootSimulator,
                    format!("simulator {device_id} did not report booted after boot"),
                )
                .await;
            bail!("simulator {device_id} did not report booted after boot");
        }

        Ok(())
    }

    async fn run_hook_for_slot(
        &self,
        project_id: &str,
        device_key: &str,
        lease_owner: &str,
        status: SlotStatus,
        phase: SlotPhase,
        program: &std::path::Path,
        timeout_sec: u64,
        payload: serde_json::Value,
        success_status: Option<SlotStatus>,
    ) -> Result<SlotRuntimeState> {
        let (cwd, env) = self
            .prepare_slot_for_hook(project_id, device_key, lease_owner, &status, &phase)
            .await?;
        let hook_result = self
            .hook_runner
            .run(HookRunRequest {
                program: program.to_path_buf(),
                cwd,
                timeout: Duration::from_secs(timeout_sec),
                env,
                payload,
            })
            .await;

        match hook_result {
            Ok(output) => {
                let state = self
                    .apply_hook_success(
                        project_id,
                        device_key,
                        output.result,
                        success_status.unwrap_or(status),
                    )
                    .await?;
                let _ = output.stdout;
                let _ = output.stderr;
                Ok(state)
            }
            Err(error) => {
                warn!("hook failed for {project_id}/{device_key} at {phase:?}: {error:#}");
                self.apply_hook_failure(project_id, device_key, phase, error.to_string())
                    .await
            }
        }
    }

    async fn prepare_slot_for_hook(
        &self,
        project_id: &str,
        device_key: &str,
        lease_owner: &str,
        status: &SlotStatus,
        phase: &SlotPhase,
    ) -> Result<(std::path::PathBuf, BTreeMap<String, String>)> {
        let project = self.project(project_id)?;
        let device = project
            .devices
            .get(device_key)
            .ok_or_else(|| anyhow!("unknown device {device_key} in project {project_id}"))?;
        let runtime_dir = self.store.runtime_dir(project, &device.runtime_subdir);
        let logs_dir = self.store.logs_dir(project_id, device_key);
        std::fs::create_dir_all(&runtime_dir)?;
        std::fs::create_dir_all(&logs_dir)?;

        let mut slots = self.slots.write().await;
        let state = self.slot_mut(&mut slots, project_id, device_key)?;
        ensure_lease_owner(state, lease_owner)?;
        state.status = status.clone();
        state.phase = phase.clone();
        state.last_error = None;
        state.updated_at = iso_timestamp();
        self.store.persist_slot_state(state)?;
        let snapshot = state.clone();
        drop(slots);
        self.events.publish_slot_state("phase_started", &snapshot);

        let mut env = project.env.clone();
        env.insert("QAH_PROJECT_ID".to_string(), project.id.clone());
        env.insert("QAH_PROJECT_NAME".to_string(), project.display_name.clone());
        env.insert("QAH_DEVICE_ID".to_string(), device.device_id.clone());
        env.insert("QAH_DEVICE_KEY".to_string(), device_key.to_string());
        env.insert("QAH_DEVICE_TYPE".to_string(), "ios_sim".to_string());
        env.insert(
            "QAH_REPO_ROOT".to_string(),
            project.repo_root.display().to_string(),
        );
        env.insert(
            "QAH_RUNTIME_DIR".to_string(),
            runtime_dir.display().to_string(),
        );
        env.insert("QAH_LOG_DIR".to_string(), logs_dir.display().to_string());
        env.insert(
            "QAH_STATE_FILE".to_string(),
            self.store.slot_state_path(project_id, device_key).display().to_string(),
        );
        env.insert("QAH_LEASE_OWNER".to_string(), lease_owner.to_string());
        env.insert("QAH_OPERATION".to_string(), format!("{phase:?}"));
        Ok((runtime_dir, env))
    }

    async fn apply_hook_success(
        &self,
        project_id: &str,
        device_key: &str,
        result: crate::models::HookResult,
        success_status: SlotStatus,
    ) -> Result<SlotRuntimeState> {
        let mut slots = self.slots.write().await;
        let state = self.slot_mut(&mut slots, project_id, device_key)?;
        for (key, value) in result.artifacts {
            state.artifacts.insert(key, stringify_json_value(value));
        }
        state.processes = result
            .processes
            .into_iter()
            .map(|process| ProcessRecord {
                purpose: process.purpose,
                pid: process.pid,
                started_at: Some(iso_timestamp()),
                expected_cleanup: process.expected_cleanup,
            })
            .collect();
        state.status = success_status.clone();
        state.phase = if success_status == SlotStatus::Ready {
            SlotPhase::None
        } else {
            state.phase.clone()
        };
        if success_status == SlotStatus::Ready {
            state.last_ready_at = Some(iso_timestamp());
        }
        state.updated_at = iso_timestamp();
        self.store.persist_slot_state(state)?;
        let snapshot = state.clone();
        drop(slots);
        self.events.publish_slot_state("phase_finished", &snapshot);
        Ok(snapshot)
    }

    async fn apply_hook_failure(
        &self,
        project_id: &str,
        device_key: &str,
        phase: SlotPhase,
        message: String,
    ) -> Result<SlotRuntimeState> {
        let mut slots = self.slots.write().await;
        let state = self.slot_mut(&mut slots, project_id, device_key)?;
        state.status = SlotStatus::Failed;
        state.phase = phase;
        state.last_error = Some(ErrorRecord {
            code: "hook_failed".to_string(),
            message,
        });
        state.updated_at = iso_timestamp();
        self.store.persist_slot_state(state)?;
        let snapshot = state.clone();
        drop(slots);
        self.events.publish_slot_state("error", &snapshot);
        Ok(snapshot)
    }
}

fn ensure_lease_owner(state: &SlotRuntimeState, owner: &str) -> Result<()> {
    match &state.lease {
        Some(lease) if lease.owner == owner => Ok(()),
        Some(lease) => bail!("slot leased by {}, not {}", lease.owner, owner),
        None => bail!("slot has no lease"),
    }
}

fn reconcile_slot_processes(
    store: &StateStore,
    states: &mut BTreeMap<(String, String), SlotRuntimeState>,
) -> Result<()> {
    for state in states.values_mut() {
        let missing_process = state
            .processes
            .iter()
            .find(|process| process.pid.is_some() && !pid_is_alive(process.pid.expect("pid checked")));
        if let Some(process) = missing_process {
            state.status = SlotStatus::Failed;
            state.phase = SlotPhase::None;
            state.last_error = Some(ErrorRecord {
                code: "tracked_process_missing".to_string(),
                message: format!(
                    "tracked process {} pid {} was not alive during harness startup",
                    process.purpose,
                    process.pid.unwrap_or_default()
                ),
            });
            state.updated_at = iso_timestamp();
            store.persist_slot_state(state)?;
            continue;
        }

        if matches!(
            state.status,
            SlotStatus::BootingSimulator
                | SlotStatus::Preparing
                | SlotStatus::StartingDependencies
                | SlotStatus::StartingRuntime
                | SlotStatus::Busy
                | SlotStatus::Stopping
        ) {
            state.status = SlotStatus::Failed;
            state.last_error = Some(ErrorRecord {
                code: "interrupted_runtime".to_string(),
                message: "slot was mid-operation during harness startup".to_string(),
            });
            state.updated_at = iso_timestamp();
            store.persist_slot_state(state)?;
        }
    }
    Ok(())
}

fn pid_is_alive(pid: u32) -> bool {
    #[cfg(unix)]
    {
        // SAFETY: `kill(pid, 0)` is a standard Unix liveness probe and sends no signal.
        let result = unsafe { libc::kill(pid as i32, 0) };
        if result == 0 {
            return true;
        }
        let errno = std::io::Error::last_os_error().raw_os_error();
        return matches!(errno, Some(libc::EPERM));
    }

    #[cfg(not(unix))]
    {
        let _ = pid;
        false
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        fs,
        path::PathBuf,
        sync::{Arc, Mutex},
    };
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    use tempfile::tempdir;

    use super::*;
    use crate::{
        config::{DeviceConfig, HooksConfig, TimeoutsConfig},
        hook_runner::HookRunner,
        ios_sim::{SimulatorController, SimulatorStatus},
        models::{CommandName, DeviceType, LeaseRequest, SlotPhase},
    };

    #[derive(Default)]
    struct FakeSimController {
        statuses: Mutex<Vec<bool>>,
        boots: Mutex<Vec<String>>,
    }

    impl FakeSimController {
        fn new(sequence: &[bool]) -> Self {
            Self {
                statuses: Mutex::new(sequence.to_vec()),
                boots: Mutex::new(Vec::new()),
            }
        }

        fn boot_calls(&self) -> Vec<String> {
            self.boots.lock().expect("boots").clone()
        }
    }

    impl SimulatorController for FakeSimController {
        fn inspect(&self, device_id: &str) -> Result<SimulatorStatus> {
            let mut statuses = self.statuses.lock().expect("statuses");
            let booted = if statuses.is_empty() {
                true
            } else {
                statuses.remove(0)
            };
            Ok(SimulatorStatus {
                device_id: device_id.to_string(),
                checked: true,
                booted: Some(booted),
            })
        }

        fn boot(&self, device_id: &str) -> Result<()> {
            self.boots.lock().expect("boots").push(device_id.to_string());
            Ok(())
        }
    }

    fn write_executable(path: &std::path::Path, body: &str) {
        fs::write(path, body).expect("write script");
        #[cfg(unix)]
        {
            let mut perms = fs::metadata(path).expect("metadata").permissions();
            perms.set_mode(0o755);
            fs::set_permissions(path, perms).expect("chmod");
        }
    }

    fn test_config(root: &std::path::Path) -> HarnessConfig {
        let hooks_dir = root.join("hooks");
        fs::create_dir_all(&hooks_dir).expect("hooks dir");
        write_executable(
            &hooks_dir.join("prepare.sh"),
            r#"#!/bin/bash
echo '{"ok":true,"artifacts":{"prepared":"yes"}}'
"#,
        );
        write_executable(
            &hooks_dir.join("run.sh"),
            r#"#!/bin/bash
echo '{"ok":true,"artifacts":{"runtime":"started"},"processes":[{"purpose":"app","pid":123,"expected_cleanup":true}]}'
"#,
        );
        write_executable(
            &hooks_dir.join("ready.sh"),
            r#"#!/bin/bash
echo '{"ok":true,"artifacts":{"ready":true}}'
"#,
        );
        write_executable(
            &hooks_dir.join("teardown.sh"),
            r#"#!/bin/bash
echo '{"ok":true,"artifacts":{"teardown":"done"}}'
"#,
        );
        write_executable(
            &hooks_dir.join("command.sh"),
            r#"#!/bin/bash
payload=$(cat)
if [[ "$payload" == *'"command":"hierarchy"'* ]]; then
  echo '{"ok":true,"artifacts":{"last_command":"Hierarchy","command_ok":"yes"}}'
else
  echo '{"ok":false,"error":{"code":"unknown_command","message":"unexpected command payload"}}'
  exit 0
fi
"#,
        );

        let mut projects = BTreeMap::new();
        let mut devices = BTreeMap::new();
        devices.insert(
            "primary".to_string(),
            DeviceConfig {
                device_type: DeviceType::IosSim,
                device_id: "SIM-123".to_string(),
                name: "Primary".to_string(),
                runtime_subdir: "sim-primary".to_string(),
                boot_policy: "lazy".to_string(),
            },
        );

        projects.insert(
            "ezra".to_string(),
            ProjectConfig {
                id: "ezra".to_string(),
                display_name: "Ezra QA".to_string(),
                repo_root: root.join("repo"),
                runtime_root: root.join("runtime"),
                env: BTreeMap::new(),
                devices,
                hooks: HooksConfig {
                    prepare_source: hooks_dir.join("prepare.sh"),
                    start_dependencies: None,
                    start_runtime: hooks_dir.join("run.sh"),
                    check_readiness: hooks_dir.join("ready.sh"),
                    teardown: hooks_dir.join("teardown.sh"),
                    command: hooks_dir.join("command.sh"),
                },
                timeouts: TimeoutsConfig::default(),
            },
        );

        HarnessConfig { projects }
    }

    #[tokio::test]
    async fn start_requires_matching_lease_owner() {
        let temp = tempdir().expect("tempdir");
        let runtime = HarnessRuntime::from_parts(
            test_config(temp.path()),
            temp.path().join("state"),
            HookRunner,
            Arc::new(FakeSimController::new(&[true])),
        )
        .expect("runtime");

        let error = runtime
            .start(
                "ezra",
                "primary",
                StartRequest {
                    lease_owner: "worker-a".to_string(),
                    startup: serde_json::json!({}),
                },
            )
            .await
            .expect_err("start without lease should fail");
        assert!(error.to_string().contains("slot has no lease"));

        runtime
            .acquire_lease(
                "ezra",
                "primary",
                LeaseRequest {
                    owner: "worker-a".to_string(),
                    reason: "qa".to_string(),
                    expires_at: None,
                },
            )
            .await
            .expect("lease");

        let error = runtime
            .start(
                "ezra",
                "primary",
                StartRequest {
                    lease_owner: "worker-b".to_string(),
                    startup: serde_json::json!({}),
                },
            )
            .await
            .expect_err("wrong owner should fail");
        assert!(error.to_string().contains("slot leased by worker-a"));
    }

    #[tokio::test]
    async fn start_and_command_update_slot_phase_for_lease_owner() {
        let temp = tempdir().expect("tempdir");
        let sim = Arc::new(FakeSimController::new(&[false, true, true]));
        let runtime = HarnessRuntime::from_parts(
            test_config(temp.path()),
            temp.path().join("state"),
            HookRunner,
            sim.clone(),
        )
        .expect("runtime");

        runtime
            .acquire_lease(
                "ezra",
                "primary",
                LeaseRequest {
                    owner: "worker-a".to_string(),
                    reason: "qa".to_string(),
                    expires_at: None,
                },
            )
            .await
            .expect("lease");

        let started = runtime
            .start(
                "ezra",
                "primary",
                StartRequest {
                    lease_owner: "worker-a".to_string(),
                    startup: serde_json::json!({}),
                },
            )
            .await
            .expect("start");
        assert_eq!(started.status, SlotStatus::Ready);
        assert_eq!(started.phase, SlotPhase::None);
        assert_eq!(
            started.artifacts.get("ready"),
            Some(&"true".to_string())
        );
        assert_eq!(sim.boot_calls(), vec!["SIM-123".to_string()]);

        let commanded = runtime
            .command(
                "ezra",
                "primary",
                CommandRequest {
                    lease_owner: "worker-a".to_string(),
                    command: CommandName::Hierarchy,
                    args: serde_json::json!({}),
                },
            )
            .await
            .expect("command");
        assert_eq!(commanded.status, SlotStatus::Ready);
        assert_eq!(commanded.phase, SlotPhase::None);
        assert_eq!(
            commanded.artifacts.get("last_command"),
            Some(&"Hierarchy".to_string())
        );
        assert_eq!(sim.boot_calls(), vec!["SIM-123".to_string()]);
    }

    #[tokio::test]
    async fn lease_conflict_is_rejected_but_same_owner_can_refresh() {
        let temp = tempdir().expect("tempdir");
        let runtime = HarnessRuntime::from_parts(
            test_config(temp.path()),
            temp.path().join("state"),
            HookRunner,
            Arc::new(FakeSimController::new(&[true])),
        )
        .expect("runtime");

        runtime
            .acquire_lease(
                "ezra",
                "primary",
                LeaseRequest {
                    owner: "worker-a".to_string(),
                    reason: "qa".to_string(),
                    expires_at: None,
                },
            )
            .await
            .expect("lease");

        let error = runtime
            .acquire_lease(
                "ezra",
                "primary",
                LeaseRequest {
                    owner: "worker-b".to_string(),
                    reason: "steal".to_string(),
                    expires_at: None,
                },
            )
            .await
            .expect_err("other owner should be rejected");
        assert!(error.to_string().contains("already leased by worker-a"));

        let refreshed = runtime
            .acquire_lease(
                "ezra",
                "primary",
                LeaseRequest {
                    owner: "worker-a".to_string(),
                    reason: "refresh".to_string(),
                    expires_at: Some("later".to_string()),
                },
            )
            .await
            .expect("same owner refresh");
        let lease = refreshed.lease.expect("lease");
        assert_eq!(lease.owner, "worker-a");
        assert_eq!(lease.reason, "refresh");
        assert_eq!(lease.expires_at.as_deref(), Some("later"));
    }

    #[tokio::test]
    async fn lease_changes_persist_to_state_file() {
        let temp = tempdir().expect("tempdir");
        let state_root = temp.path().join("state");
        let runtime = HarnessRuntime::from_parts(
            test_config(temp.path()),
            state_root.clone(),
            HookRunner,
            Arc::new(FakeSimController::new(&[true])),
        )
        .expect("runtime");

        runtime
            .acquire_lease(
                "ezra",
                "primary",
                LeaseRequest {
                    owner: "worker-a".to_string(),
                    reason: "qa".to_string(),
                    expires_at: None,
                },
            )
            .await
            .expect("lease");

        let persisted = std::fs::read_to_string(
            state_root.join("state").join("ezra").join("primary.json"),
        )
        .expect("persisted state");
        assert!(persisted.contains("\"owner\": \"worker-a\""));
        assert!(persisted.contains("\"status\": \"idle\""));
    }

    #[tokio::test]
    async fn command_does_not_boot_when_simulator_already_booted() {
        let temp = tempdir().expect("tempdir");
        let sim = Arc::new(FakeSimController::new(&[true]));
        let runtime = HarnessRuntime::from_parts(
            test_config(temp.path()),
            temp.path().join("state"),
            HookRunner,
            sim.clone(),
        )
        .expect("runtime");

        runtime
            .acquire_lease(
                "ezra",
                "primary",
                LeaseRequest {
                    owner: "worker-a".to_string(),
                    reason: "qa".to_string(),
                    expires_at: None,
                },
            )
            .await
            .expect("lease");

        let commanded = runtime
            .command(
                "ezra",
                "primary",
                CommandRequest {
                    lease_owner: "worker-a".to_string(),
                    command: CommandName::Hierarchy,
                    args: serde_json::json!({}),
                },
            )
            .await
            .expect("command");

        assert_eq!(commanded.status, SlotStatus::Ready);
        assert!(sim.boot_calls().is_empty());
    }

    #[tokio::test]
    async fn lease_acquire_emits_event() {
        let temp = tempdir().expect("tempdir");
        let runtime = HarnessRuntime::from_parts(
            test_config(temp.path()),
            temp.path().join("state"),
            HookRunner,
            Arc::new(FakeSimController::new(&[true])),
        )
        .expect("runtime");
        let mut events = runtime.subscribe_events();

        runtime
            .acquire_lease(
                "ezra",
                "primary",
                LeaseRequest {
                    owner: "worker-a".to_string(),
                    reason: "qa".to_string(),
                    expires_at: None,
                },
            )
            .await
            .expect("lease");

        let event = events.recv().await.expect("event");
        assert_eq!(event.kind, "lease_acquired");
        assert_eq!(event.project_id, "ezra");
        assert_eq!(event.device_key, "primary");
        assert_eq!(
            event.state.lease.as_ref().map(|lease| lease.owner.as_str()),
            Some("worker-a")
        );
    }

    #[tokio::test]
    async fn startup_reconciles_missing_tracked_process_to_failed() {
        let temp = tempdir().expect("tempdir");
        let state_root = temp.path().join("state");
        let store = StateStore::new(state_root.clone());
        store.ensure_layout().expect("layout");
        store
            .persist_slot_state(&SlotRuntimeState {
                project_id: "ezra".to_string(),
                device_key: "primary".to_string(),
                device_id: "SIM-123".to_string(),
                runtime_dir: temp.path().join("runtime"),
                status: SlotStatus::Ready,
                phase: SlotPhase::None,
                lease: Some(LeaseRecord {
                    owner: "worker-a".to_string(),
                    reason: "qa".to_string(),
                    acquired_at: "1".to_string(),
                    expires_at: None,
                }),
                artifacts: BTreeMap::new(),
                processes: vec![ProcessRecord {
                    purpose: "app".to_string(),
                    pid: Some(999_999),
                    started_at: Some("1".to_string()),
                    expected_cleanup: true,
                }],
                last_error: None,
                last_ready_at: Some("1".to_string()),
                updated_at: "1".to_string(),
            })
            .expect("persist");

        let runtime = HarnessRuntime::from_parts(
            test_config(temp.path()),
            state_root,
            HookRunner,
            Arc::new(FakeSimController::new(&[true])),
        )
        .expect("runtime");

        let summary = runtime.device_summary("ezra", "primary").await.expect("summary");
        assert_eq!(summary.state.status, SlotStatus::Failed);
        assert_eq!(
            summary.state.last_error.as_ref().expect("error").code,
            "tracked_process_missing"
        );
    }

    #[tokio::test]
    async fn startup_reconciles_inflight_state_to_failed() {
        let temp = tempdir().expect("tempdir");
        let state_root = temp.path().join("state");
        let store = StateStore::new(state_root.clone());
        store.ensure_layout().expect("layout");
        store
            .persist_slot_state(&SlotRuntimeState {
                project_id: "ezra".to_string(),
                device_key: "primary".to_string(),
                device_id: "SIM-123".to_string(),
                runtime_dir: temp.path().join("runtime"),
                status: SlotStatus::Busy,
                phase: SlotPhase::ExecuteCommand,
                lease: None,
                artifacts: BTreeMap::new(),
                processes: Vec::new(),
                last_error: None,
                last_ready_at: None,
                updated_at: "1".to_string(),
            })
            .expect("persist");

        let runtime = HarnessRuntime::from_parts(
            test_config(temp.path()),
            state_root,
            HookRunner,
            Arc::new(FakeSimController::new(&[true])),
        )
        .expect("runtime");

        let summary = runtime.device_summary("ezra", "primary").await.expect("summary");
        assert_eq!(summary.state.status, SlotStatus::Failed);
        assert_eq!(
            summary.state.last_error.as_ref().expect("error").code,
            "interrupted_runtime"
        );
    }

    #[test]
    fn stringify_json_value_preserves_strings_and_serializes_objects() {
        assert_eq!(stringify_json_value(serde_json::json!("hi")), "hi");
        assert_eq!(stringify_json_value(serde_json::json!(true)), "true");
        assert_eq!(
            stringify_json_value(serde_json::json!({"a":1})),
            r#"{"a":1}"#
        );
    }
}

fn stringify_json_value(value: serde_json::Value) -> String {
    match value {
        serde_json::Value::String(inner) => inner,
        other => other.to_string(),
    }
}

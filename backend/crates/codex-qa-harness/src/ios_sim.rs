use std::{process::Command, sync::Arc};

use anyhow::{Context, Result, bail};
use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SimulatorStatus {
    pub device_id: String,
    pub checked: bool,
    pub booted: Option<bool>,
}

pub trait SimulatorController: Send + Sync {
    fn inspect(&self, device_id: &str) -> Result<SimulatorStatus>;
    fn boot(&self, device_id: &str) -> Result<()>;
}

#[derive(Debug, Default)]
pub struct SimctlController;

impl SimulatorController for SimctlController {
    fn inspect(&self, device_id: &str) -> Result<SimulatorStatus> {
        let output = Command::new("xcrun")
            .args(["simctl", "list", "devices", device_id, "--json"])
            .output();

        match output {
            Ok(command) => {
                let stdout = String::from_utf8_lossy(&command.stdout);
                let booted = stdout.contains("\"state\" : \"Booted\"")
                    || stdout.contains("\"state\":\"Booted\"");
                Ok(SimulatorStatus {
                    device_id: device_id.to_string(),
                    checked: true,
                    booted: Some(booted),
                })
            }
            Err(_) => Ok(SimulatorStatus {
                device_id: device_id.to_string(),
                checked: false,
                booted: None,
            }),
        }
    }

    fn boot(&self, device_id: &str) -> Result<()> {
        let boot = Command::new("xcrun")
            .args(["simctl", "boot", device_id])
            .output()
            .with_context(|| format!("boot simulator {device_id}"))?;
        if !boot.status.success() {
            let stderr = String::from_utf8_lossy(&boot.stderr);
            if !stderr.contains("Unable to boot device in current state: Booted") {
                bail!("boot simulator {device_id} failed: {}", stderr.trim());
            }
        }

        let bootstatus = Command::new("xcrun")
            .args(["simctl", "bootstatus", device_id, "-b"])
            .output()
            .with_context(|| format!("wait for simulator {device_id} bootstatus"))?;
        if !bootstatus.status.success() {
            bail!(
                "bootstatus simulator {device_id} failed: {}",
                String::from_utf8_lossy(&bootstatus.stderr).trim()
            );
        }
        Ok(())
    }
}

pub type SharedSimulatorController = Arc<dyn SimulatorController>;

pub fn default_controller() -> SharedSimulatorController {
    Arc::new(SimctlController)
}


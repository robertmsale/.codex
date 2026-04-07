use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use serde::Deserialize;

#[derive(Debug, Parser)]
#[command(name = "codex-supervisor")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Inventory {
        #[arg(long, default_value = "supervisor/services.toml")]
        file: PathBuf,
    },
}

#[derive(Debug, Deserialize)]
struct ServiceFile {
    service: Vec<ServiceEntry>,
}

#[derive(Debug, Deserialize)]
struct ServiceEntry {
    name: String,
    #[serde(rename = "crate")]
    crate_name: Option<String>,
    current_runner: String,
    current_supervisor: String,
    port: u16,
    status: String,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Inventory { file } => inventory(file),
    }
}

fn inventory(file: PathBuf) -> Result<()> {
    let raw = std::fs::read_to_string(&file)
        .with_context(|| format!("failed to read service inventory at {}", file.display()))?;
    let inventory: ServiceFile = toml::from_str(&raw)
        .with_context(|| format!("failed to parse service inventory at {}", file.display()))?;

    for service in inventory.service {
        let crate_name = service.crate_name.unwrap_or_else(|| "-".to_string());
        println!(
            "{} | crate={} | status={} | port={} | runner={} | supervisor={}",
            service.name,
            crate_name,
            service.status,
            service.port,
            service.current_runner,
            service.current_supervisor
        );
    }
    Ok(())
}

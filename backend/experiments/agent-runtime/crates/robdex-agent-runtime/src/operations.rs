use std::net::SocketAddr;

use anyhow::{Context, Result, bail};
use serde::Serialize;
use serde_json::json;
use sqlx::{PgPool, Row};

use crate::{command_registry, db, roles, starlark_host};

#[derive(Debug, Clone)]
pub struct ResidentServerConfig {
    pub database_url: String,
    pub bind_host: String,
    pub bind_port: u16,
    pub runtime_identity: String,
    pub schema_initialization: SchemaInitializationPolicy,
    pub seed_roles: SeedRolePolicy,
    pub command_bootstrap: CommandBootstrapPolicy,
    pub process_reconciliation: ProcessReconciliationPolicy,
    pub shutdown: ShutdownPolicy,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SchemaInitializationPolicy {
    Apply,
    Skip,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SeedRolePolicy {
    ImportSeeds,
    Skip,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CommandBootstrapPolicy {
    BootstrapDefaults,
    Skip,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ProcessReconciliationPolicy {
    MarkRunningLost,
    Skip,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ShutdownPolicy {
    GracefulMarkRunningLost,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartupReport {
    pub runtime_identity: String,
    pub database_target: String,
    pub database_identity: String,
    pub bind_address: String,
    pub policies: StartupPoliciesReport,
    pub schema_applied: bool,
    pub seed_roles_imported: usize,
    pub seed_roles_unchanged: usize,
    pub command_bootstrap_applied: bool,
    pub reconciliation: ReconciliationReport,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartupPoliciesReport {
    pub schema_initialization: SchemaInitializationPolicy,
    pub seed_roles: SeedRolePolicy,
    pub command_bootstrap: CommandBootstrapPolicy,
    pub process_reconciliation: ProcessReconciliationPolicy,
    pub shutdown: ShutdownPolicy,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReconciliationReport {
    pub reason: String,
    pub lost_processes: u64,
    pub process_events: u64,
    pub session_events: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShutdownReport {
    pub runtime_identity: String,
    pub policy: ShutdownPolicy,
    pub live_processes_terminated: usize,
    pub reconciliation: ReconciliationReport,
    pub database_pool_closed: bool,
}

impl ResidentServerConfig {
    pub fn bind_address(&self) -> Result<SocketAddr> {
        format!("{}:{}", self.bind_host, self.bind_port)
            .parse()
            .with_context(|| format!("invalid bind address {}:{}", self.bind_host, self.bind_port))
    }
}

pub async fn startup(pool: &PgPool, config: &ResidentServerConfig) -> Result<StartupReport> {
    let database_identity = database_identity(pool).await.unwrap_or_else(|error| format!("unavailable: {error}"));
    let mut schema_applied = false;
    if matches!(config.schema_initialization, SchemaInitializationPolicy::Apply) {
        db::apply_schema(pool).await?;
        schema_applied = true;
    }

    let mut command_bootstrap_applied = false;
    if matches!(config.command_bootstrap, CommandBootstrapPolicy::BootstrapDefaults) {
        command_registry::bootstrap_seed_defaults(pool).await?;
        command_bootstrap_applied = true;
    }

    let seed_roles = match config.seed_roles {
        SeedRolePolicy::ImportSeeds => import_seed_roles(pool).await?,
        SeedRolePolicy::Skip => SeedRoleImportReport { imported: 0, unchanged: 0 },
    };

    let reconciliation = match config.process_reconciliation {
        ProcessReconciliationPolicy::MarkRunningLost => {
            let summary = db::reconcile_managed_processes(pool, "runtimeRestart").await?;
            ReconciliationReport {
                reason: "runtimeRestart".to_string(),
                lost_processes: summary.lost_processes,
                process_events: summary.process_events,
                session_events: summary.session_events,
            }
        }
        ProcessReconciliationPolicy::Skip => ReconciliationReport {
            reason: "skipped".to_string(),
            lost_processes: 0,
            process_events: 0,
            session_events: 0,
        },
    };

    Ok(StartupReport {
        runtime_identity: config.runtime_identity.clone(),
        database_target: redacted_database_target(&config.database_url),
        database_identity,
        bind_address: config.bind_address()?.to_string(),
        policies: StartupPoliciesReport {
            schema_initialization: config.schema_initialization,
            seed_roles: config.seed_roles,
            command_bootstrap: config.command_bootstrap,
            process_reconciliation: config.process_reconciliation,
            shutdown: config.shutdown,
        },
        schema_applied,
        seed_roles_imported: seed_roles.imported,
        seed_roles_unchanged: seed_roles.unchanged,
        command_bootstrap_applied,
        reconciliation,
    })
}

pub async fn shutdown(pool: &PgPool, config: &ResidentServerConfig) -> Result<ShutdownReport> {
    match config.shutdown {
        ShutdownPolicy::GracefulMarkRunningLost => {
            let live_processes_terminated = starlark_host::terminate_all_runtime_processes("runtimeShutdown");
            let summary = db::reconcile_managed_processes(pool, "runtimeShutdown").await?;
            pool.close().await;
            Ok(ShutdownReport {
                runtime_identity: config.runtime_identity.clone(),
                policy: config.shutdown,
                live_processes_terminated,
                reconciliation: ReconciliationReport {
                    reason: "runtimeShutdown".to_string(),
                    lost_processes: summary.lost_processes,
                    process_events: summary.process_events,
                    session_events: summary.session_events,
                },
                database_pool_closed: true,
            })
        }
    }
}

pub fn print_startup_report(report: &StartupReport) {
    println!("[server-startup] {}", serde_json::to_string(&json!(report)).unwrap_or_else(|_| "{\"error\":\"serialize startup report\"}".to_string()));
}

pub fn print_shutdown_report(report: &ShutdownReport) {
    println!("[server-shutdown] {}", serde_json::to_string(&json!(report)).unwrap_or_else(|_| "{\"error\":\"serialize shutdown report\"}".to_string()));
}

#[derive(Debug, Clone, Copy, Default)]
struct SeedRoleImportReport {
    imported: usize,
    unchanged: usize,
}

async fn import_seed_roles(pool: &PgPool) -> Result<SeedRoleImportReport> {
    let registry = roles::RoleRegistry::default_for_workspace()?;
    let mut report = SeedRoleImportReport::default();
    for path in registry.manifest_paths()? {
        let role = registry.load_for_import(&path)?;
        if current_role_content_matches(pool, &role).await? {
            report.unchanged += 1;
        } else {
            db::import_role_version(pool, &role).await?;
            report.imported += 1;
        }
    }
    Ok(report)
}

async fn current_role_content_matches(pool: &PgPool, imported: &roles::ImportedRoleVersion) -> Result<bool> {
    let snapshot = &imported.snapshot;
    let row = sqlx::query(
        r#"
        SELECT rv.version, rv.display_name, rv.instruction_text, rv.manifest,
               rv.model_defaults, rv.policy, rv.routing, rv.visibility, rv.lifecycle_authority
        FROM roles r
        JOIN role_versions rv ON rv.id = r.current_version_id
        WHERE r.id = $1
        "#,
    )
    .bind(&snapshot.id)
    .fetch_optional(pool)
    .await?;
    let Some(row) = row else {
        return Ok(false);
    };
    Ok(row.get::<String, _>("version") == snapshot.version
        && row.get::<String, _>("display_name") == snapshot.display_name
        && row.get::<String, _>("instruction_text") == snapshot.instruction_text
        && row.get::<serde_json::Value, _>("manifest") == imported.manifest_json
        && row.get::<serde_json::Value, _>("model_defaults") == serde_json::to_value(&snapshot.model_defaults)?
        && row.get::<serde_json::Value, _>("policy") == serde_json::to_value(&snapshot.policy)?
        && row.get::<serde_json::Value, _>("routing") == serde_json::to_value(&snapshot.routing)?
        && row.get::<serde_json::Value, _>("visibility") == serde_json::to_value(&snapshot.visibility)?
        && row.get::<serde_json::Value, _>("lifecycle_authority") == serde_json::to_value(&snapshot.lifecycle_authority)?)
}

async fn database_identity(pool: &PgPool) -> Result<String> {
    let row = sqlx::query("SELECT current_database() AS database, inet_server_addr()::text AS host, inet_server_port() AS port")
        .fetch_one(pool)
        .await?;
    let database: String = row.get("database");
    let host: Option<String> = row.get("host");
    let port: Option<i32> = row.get("port");
    Ok(format!("{}@{}:{}", database, host.unwrap_or_else(|| "local".to_string()), port.map(|value| value.to_string()).unwrap_or_else(|| "local".to_string())))
}

fn redacted_database_target(database_url: &str) -> String {
    let Some((scheme, rest)) = database_url.split_once("://") else {
        return database_url.to_string();
    };
    if let Some(at) = rest.rfind('@') {
        format!("{scheme}://<redacted>@{}", &rest[at + 1..])
    } else {
        database_url.to_string()
    }
}

pub fn parse_schema_policy(value: &str) -> Result<SchemaInitializationPolicy> {
    match value {
        "apply" => Ok(SchemaInitializationPolicy::Apply),
        "skip" => Ok(SchemaInitializationPolicy::Skip),
        other => bail!("unsupported schema initialization policy: {other}"),
    }
}

pub fn parse_seed_role_policy(value: &str) -> Result<SeedRolePolicy> {
    match value {
        "importSeeds" | "import-seeds" => Ok(SeedRolePolicy::ImportSeeds),
        "skip" => Ok(SeedRolePolicy::Skip),
        other => bail!("unsupported seed role policy: {other}"),
    }
}

pub fn parse_command_bootstrap_policy(value: &str) -> Result<CommandBootstrapPolicy> {
    match value {
        "bootstrapDefaults" | "bootstrap-defaults" => Ok(CommandBootstrapPolicy::BootstrapDefaults),
        "skip" => Ok(CommandBootstrapPolicy::Skip),
        other => bail!("unsupported command bootstrap policy: {other}"),
    }
}

pub fn parse_process_reconciliation_policy(value: &str) -> Result<ProcessReconciliationPolicy> {
    match value {
        "markRunningLost" | "mark-running-lost" => Ok(ProcessReconciliationPolicy::MarkRunningLost),
        "skip" => Ok(ProcessReconciliationPolicy::Skip),
        other => bail!("unsupported process reconciliation policy: {other}"),
    }
}

pub fn parse_shutdown_policy(value: &str) -> Result<ShutdownPolicy> {
    match value {
        "gracefulMarkRunningLost" | "graceful-mark-running-lost" => Ok(ShutdownPolicy::GracefulMarkRunningLost),
        other => bail!("unsupported shutdown policy: {other}"),
    }
}

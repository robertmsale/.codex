use anyhow::Result;
use clap::Parser;
use robdex_agent_runtime::{db, operations, server};

const DEFAULT_DATABASE_URL: &str =
    "postgres://postgres:postgres@127.0.0.1:5432/robdex_agent_runtime";

#[derive(Debug, Parser)]
#[command(name = "robdex-agent-runtime-server")]
struct Cli {
    #[arg(long, env = "ROBDEX_AGENT_RUNTIME_DATABASE_URL", default_value = DEFAULT_DATABASE_URL)]
    database_url: String,
    #[arg(long, env = "ROBDEX_AGENT_RUNTIME_SERVER_HOST", default_value = "127.0.0.1")]
    host: String,
    #[arg(long, env = "ROBDEX_AGENT_RUNTIME_SERVER_PORT", default_value_t = 8765)]
    port: u16,
    #[arg(long, env = "ROBDEX_AGENT_RUNTIME_IDENTITY")]
    runtime_identity: Option<String>,
    #[arg(long, env = "ROBDEX_AGENT_RUNTIME_SCHEMA_POLICY", default_value = "apply")]
    schema_policy: String,
    #[arg(long, env = "ROBDEX_AGENT_RUNTIME_SEED_ROLE_POLICY", default_value = "importSeeds")]
    seed_role_policy: String,
    #[arg(long, env = "ROBDEX_AGENT_RUNTIME_COMMAND_BOOTSTRAP_POLICY", default_value = "bootstrapDefaults")]
    command_bootstrap_policy: String,
    #[arg(long, env = "ROBDEX_AGENT_RUNTIME_PROCESS_RECONCILIATION_POLICY", default_value = "markRunningLost")]
    process_reconciliation_policy: String,
    #[arg(long, env = "ROBDEX_AGENT_RUNTIME_SHUTDOWN_POLICY", default_value = "gracefulMarkRunningLost")]
    shutdown_policy: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let config = operations::ResidentServerConfig {
        database_url: cli.database_url,
        bind_host: cli.host,
        bind_port: cli.port,
        runtime_identity: cli.runtime_identity.unwrap_or_else(|| format!("robdex-agent-runtime/{}", env!("CARGO_PKG_VERSION"))),
        schema_initialization: operations::parse_schema_policy(&cli.schema_policy)?,
        seed_roles: operations::parse_seed_role_policy(&cli.seed_role_policy)?,
        command_bootstrap: operations::parse_command_bootstrap_policy(&cli.command_bootstrap_policy)?,
        process_reconciliation: operations::parse_process_reconciliation_policy(&cli.process_reconciliation_policy)?,
        shutdown: operations::parse_shutdown_policy(&cli.shutdown_policy)?,
    };
    let pool = db::connect(&config.database_url).await?;
    let report = operations::startup(&pool, &config).await?;
    operations::print_startup_report(&report);
    server::serve_with_shutdown(
        pool.clone(),
        &config.bind_host,
        config.bind_port,
        config.runtime_identity.clone(),
        shutdown_signal(),
    )
    .await?;
    let shutdown_report = operations::shutdown(&pool, &config).await?;
    operations::print_shutdown_report(&shutdown_report);
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(error) = tokio::signal::ctrl_c().await {
            eprintln!("[server-shutdown] ctrl_c signal registration failed: {error}");
        }
    };
    #[cfg(unix)]
    {
        let terminate = async {
            match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
                Ok(mut signal) => {
                    signal.recv().await;
                }
                Err(error) => {
                    eprintln!("[server-shutdown] terminate signal registration failed: {error}");
                    std::future::pending::<()>().await;
                }
            }
        };
        tokio::select! {
            _ = ctrl_c => {},
            _ = terminate => {},
        }
    }
    #[cfg(not(unix))]
    {
        ctrl_c.await;
    }
}

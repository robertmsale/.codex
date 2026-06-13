use anyhow::Result;
use clap::Parser;
use robdex_agent_runtime::{db, server};

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
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let pool = db::connect(&cli.database_url).await?;
    db::init(&pool).await?;
    server::serve(pool, &cli.host, cli.port).await
}

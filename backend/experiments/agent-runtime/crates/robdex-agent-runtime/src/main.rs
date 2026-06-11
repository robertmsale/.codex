use anyhow::Result;
use clap::{Parser, Subcommand};
use uuid::Uuid;

use robdex_agent_runtime::{db, runtime};
use robdex_agent_runtime::roles::{DEFAULT_ROLE_ID, RoleRegistry};

const DEFAULT_DATABASE_URL: &str =
    "postgres://postgres:postgres@127.0.0.1:5432/robdex_agent_runtime";

#[derive(Debug, Parser)]
#[command(name = "robdex-agent-runtime")]
struct Cli {
    #[arg(long, env = "ROBDEX_AGENT_RUNTIME_DATABASE_URL", default_value = DEFAULT_DATABASE_URL)]
    database_url: String,

    #[arg(long, default_value = ".")]
    workdir: String,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    InitDb,
    NewSession {
        #[arg(long, default_value = DEFAULT_ROLE_ID)]
        role: String,
    },
    Send {
        #[arg(long)]
        session: Uuid,
        #[arg(long)]
        message: String,
    },
    Events {
        #[arg(long)]
        session: Uuid,
    },
    Roles {
        #[command(subcommand)]
        command: RolesCommand,
    },
}

#[derive(Debug, Subcommand)]
enum RolesCommand {
    List,
    Validate {
        #[arg(long)]
        manifest: Option<std::path::PathBuf>,
    },
    Show {
        id: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let pool = db::connect(&cli.database_url).await?;
    match cli.command {
        Command::InitDb => {
            db::init(&pool).await?;
            println!("initialized experimental Postgres schema");
        }
        Command::NewSession { role } => {
            let registry = RoleRegistry::default_for_workspace()?;
            let snapshot = registry.snapshot(&role)?;
            let id = db::new_session(&pool, &snapshot).await?;
            println!("{id}");
        }
        Command::Send { session, message } => {
            runtime::send(&pool, session, &message, &cli.workdir).await?;
        }
        Command::Events { session } => {
            db::print_events(&pool, session).await?;
        }
        Command::Roles { command } => {
            let registry = RoleRegistry::default_for_workspace()?;
            match command {
                RolesCommand::List => {
                    for role in registry.validate_all()? {
                        println!("{} {} {}", role.id, role.version, role.display_name);
                    }
                }
                RolesCommand::Validate { manifest } => {
                    if let Some(path) = manifest {
                        let role = registry.load_path(&path)?;
                        println!("valid {}", role.id);
                    } else {
                        let roles = registry.validate_all()?;
                        println!("valid {} role manifests", roles.len());
                    }
                }
                RolesCommand::Show { id } => {
                    let role = registry.load(&id)?;
                    println!("{}", serde_json::to_string_pretty(&role)?);
                }
            }
        }
    }
    Ok(())
}

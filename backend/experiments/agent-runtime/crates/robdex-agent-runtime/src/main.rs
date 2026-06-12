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
    Import {
        manifest: std::path::PathBuf,
    },
    ImportSeeds,
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
            let snapshot = db::current_role_snapshot(&pool, &role).await?;
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
                RolesCommand::Import { manifest } => {
                    let imported = registry.load_for_import(&manifest)?;
                    db::import_role_version(&pool, &imported).await?;
                    println!(
                        "imported {} {} {}",
                        imported.snapshot.id, imported.snapshot.version, imported.snapshot.role_version_id
                    );
                }
                RolesCommand::ImportSeeds => {
                    let mut count = 0usize;
                    for path in registry.manifest_paths()? {
                        let imported = registry.load_for_import(&path)?;
                        db::import_role_version(&pool, &imported).await?;
                        count += 1;
                    }
                    println!("imported {count} seed roles");
                }
                RolesCommand::List => {
                    for role in db::list_roles(&pool).await? {
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
                    let role = db::current_role_snapshot(&pool, &id).await?;
                    println!("{}", serde_json::to_string_pretty(&role)?);
                }
            }
        }
    }
    Ok(())
}

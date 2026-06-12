use anyhow::Result;
use clap::{Parser, Subcommand};
use uuid::Uuid;
use std::collections::BTreeSet;

use robdex_agent_runtime::{approvals, command_registry, db, routing, runtime};
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
    Approvals {
        #[command(subcommand)]
        command: ApprovalsCommand,
    },
    CommandRegistry {
        #[command(subcommand)]
        command: CommandRegistryCommand,
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

#[derive(Debug, Subcommand)]
enum ApprovalsCommand {
    List,
    Show {
        id: Uuid,
    },
    Decide {
        id: Uuid,
        #[arg(long)]
        decision: String,
        #[arg(long)]
        reason: String,
    },
    Resume {
        id: Uuid,
    },
}

#[derive(Debug, Subcommand)]
enum CommandRegistryCommand {
    List,
    Show { action_id: String },
    Requests {
        #[command(subcommand)]
        command: CommandRegistryRequestCommand,
    },
}

#[derive(Debug, Subcommand)]
enum CommandRegistryRequestCommand {
    Create {
        #[arg(long)]
        session: Uuid,
        json_file: std::path::PathBuf,
    },
    List,
    Show { id: Uuid },
    Decide {
        #[arg(long)]
        session: Uuid,
        id: Uuid,
        #[arg(long)]
        status: String,
    },
    Apply {
        #[arg(long)]
        session: Uuid,
        id: Uuid,
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
                    routing::validate_manifest_against_db(&pool, &imported.manifest).await?;
                    command_registry::validate_policy_actions_exist(&pool, imported.snapshot.policy.keys().cloned()).await?;
                    db::import_role_version(&pool, &imported).await?;
                    println!(
                        "imported {} {} {}",
                        imported.snapshot.id, imported.snapshot.version, imported.snapshot.role_version_id
                    );
                }
                RolesCommand::ImportSeeds => {
                    let mut count = 0usize;
                    let paths = registry.manifest_paths()?;
                    let mut imports = Vec::new();
                    let mut context = BTreeSet::new();
                    for path in &paths {
                        let imported = registry.load_for_import(path)?;
                        context.insert(imported.snapshot.id.clone());
                        imports.push(imported);
                    }
                    for imported in &imports {
                        routing::validate_routing(&imported.manifest.routing, Some(&pool), &context).await?;
                        command_registry::validate_policy_actions_exist(&pool, imported.snapshot.policy.keys().cloned()).await?;
                    }
                    for imported in imports {
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
                        let imported = registry.load_for_import(&path)?;
                        routing::validate_manifest_against_db(&pool, &imported.manifest).await?;
                        command_registry::validate_policy_actions_exist(&pool, imported.snapshot.policy.keys().cloned()).await?;
                        println!("valid {}", imported.snapshot.id);
                    } else {
                        let paths = registry.manifest_paths()?;
                        let mut imports = Vec::new();
                        let mut context = BTreeSet::new();
                        for path in &paths {
                            let imported = registry.load_for_import(path)?;
                            context.insert(imported.snapshot.id.clone());
                            imports.push(imported);
                        }
                        for imported in &imports {
                            routing::validate_routing(&imported.manifest.routing, Some(&pool), &context).await?;
                            command_registry::validate_policy_actions_exist(&pool, imported.snapshot.policy.keys().cloned()).await?;
                        }
                        println!("valid {} role manifests", imports.len());
                    }
                }
                RolesCommand::Show { id } => {
                    let role = db::current_role_snapshot(&pool, &id).await?;
                    println!("{}", serde_json::to_string_pretty(&role)?);
                }
            }
        }
        Command::Approvals { command } => match command {
            ApprovalsCommand::List => {
                for approval in approvals::list(&pool).await? {
                    println!("{}", serde_json::to_string(&approval)?);
                }
            }
            ApprovalsCommand::Show { id } => {
                println!("{}", serde_json::to_string_pretty(&approvals::show(&pool, id).await?)?);
            }
            ApprovalsCommand::Decide { id, decision, reason } => {
                let decision = approvals::ApprovalDecision::try_from(decision.as_str())?;
                approvals::decide(&pool, id, decision, &reason).await?;
                println!("decided {id} {}", decision.as_str());
            }
            ApprovalsCommand::Resume { id } => {
                approvals::resume(&pool, id).await?;
                println!("resumed {id}");
            }
        },
        Command::CommandRegistry { command } => match command {
            CommandRegistryCommand::List => {
                println!("{}", serde_json::to_string_pretty(&command_registry::list(&pool).await?)?);
            }
            CommandRegistryCommand::Show { action_id } => {
                println!("{}", serde_json::to_string_pretty(&command_registry::show(&pool, &action_id).await?)?);
            }
            CommandRegistryCommand::Requests { command } => match command {
                CommandRegistryRequestCommand::Create { session, json_file } => {
                    let raw = std::fs::read_to_string(&json_file)?;
                    let input: command_registry::ChangeRequestInput = serde_json::from_str(&raw)?;
                    let id = command_registry::create_request(&pool, session, input).await?;
                    println!("{id}");
                }
                CommandRegistryRequestCommand::List => {
                    println!("{}", serde_json::to_string_pretty(&command_registry::list_requests(&pool).await?)?);
                }
                CommandRegistryRequestCommand::Show { id } => {
                    println!("{}", serde_json::to_string_pretty(&command_registry::show_request(&pool, id).await?)?);
                }
                CommandRegistryRequestCommand::Decide { session, id, status } => {
                    command_registry::decide_request(&pool, session, id, &status).await?;
                    println!("command registry request {id} {status}");
                }
                CommandRegistryRequestCommand::Apply { session, id } => {
                    command_registry::apply_request(&pool, session, id).await?;
                    println!("applied command registry request {id}");
                }
            },
        },
    }
    Ok(())
}

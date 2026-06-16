use anyhow::Result;
use clap::{Parser, Subcommand};
use uuid::Uuid;
use std::collections::BTreeSet;

use robdex_agent_runtime::{approvals, command_registry, compaction, db, routing, runtime};
use robdex_agent_runtime::roles::{DEFAULT_ROLE_ID, RoleRegistry};

const DEFAULT_DATABASE_URL: &str =
    "postgres://postgres:postgres@127.0.0.1:5432/robdex_agent_runtime";

#[derive(Debug, Parser)]
#[command(name = "robdex-agent-runtime")]
struct Cli {
    #[arg(long, env = "ROBDEX_AGENT_RUNTIME_DATABASE_URL", default_value = DEFAULT_DATABASE_URL)]
    database_url: String,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    InitDb,
    Sessions {
        #[command(subcommand)]
        command: SessionsCommand,
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
    Compact {
        #[arg(long)]
        session: Uuid,
        #[arg(long = "through-turn")]
        through_turn: Option<Uuid>,
    },
}


#[derive(Debug, Subcommand)]
enum SessionsCommand {
    New {
        #[arg(long, default_value = DEFAULT_ROLE_ID)]
        role: String,
        #[arg(long)]
        project: Option<String>,
        #[arg(long, default_value = ".")]
        workdir: String,
        #[arg(long)]
        worktree_root: Option<String>,
        #[arg(long)]
        title: Option<String>,
        #[arg(long)]
        name: Option<String>,
    },
    List {
        #[arg(long)]
        all: bool,
    },
    Show {
        id: Uuid,
    },
    History {
        id: Uuid,
    },
    Close {
        id: Uuid,
        #[arg(long, default_value = "closed by operator")]
        reason: String,
    },
    Archive {
        id: Uuid,
    },
    Fork {
        id: Uuid,
        #[arg(long = "at-turn")]
        at_turn: Uuid,
    },
}

#[derive(Debug, Subcommand)]
enum RolesCommand {
    Create {
        manifest: std::path::PathBuf,
    },
    Update {
        manifest: std::path::PathBuf,
    },
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
    Versions {
        id: String,
    },
    Version {
        id: Uuid,
    },
    Activate {
        id: String,
        #[arg(long = "version-id")]
        version_id: Uuid,
    },
    Archive {
        id: String,
    },
    Unarchive {
        id: String,
    },
    Export {
        id: String,
        #[arg(long)]
        out: Option<std::path::PathBuf>,
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
    SeedRequests {
        #[arg(long)]
        session: Uuid,
        #[arg(long, default_value = "missing")]
        mode: String,
    },
    Requests {
        #[command(subcommand)]
        command: CommandRegistryRequestCommand,
    },
}

#[derive(Debug, Subcommand)]
enum CommandRegistryRequestCommand {
    List,
    Show { id: Uuid },
    Review { id: Uuid },
    FinalTemplate {
        id: Uuid,
        #[arg(long)]
        out: Option<std::path::PathBuf>,
    },
    PreviewDecision {
        id: Uuid,
        #[arg(long)]
        status: String,
        #[arg(long)]
        final_scope: Option<String>,
        #[arg(long)]
        final_project: Option<String>,
        #[arg(long)]
        final_policy: Option<String>,
        #[arg(long)]
        final_command_file: Option<std::path::PathBuf>,
    },
    Decide {
        #[arg(long)]
        session: Uuid,
        id: Uuid,
        #[arg(long)]
        status: String,
        #[arg(long)]
        final_scope: Option<String>,
        #[arg(long)]
        final_project: Option<String>,
        #[arg(long)]
        final_policy: Option<String>,
        #[arg(long)]
        final_command_file: Option<std::path::PathBuf>,
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
        Command::Sessions { command } => match command {
            SessionsCommand::New { role, project, workdir, worktree_root, title, name } => {
                let snapshot = db::current_role_snapshot(&pool, &role).await?;
                let id = db::new_session(&pool, &snapshot, project.as_deref(), &workdir, worktree_root.as_deref(), title.as_deref(), name.as_deref()).await?;
                println!("{id}");
            }
            SessionsCommand::List { all } => {
                for session in db::list_sessions(&pool, all).await? {
                    println!("{}", serde_json::to_string(&session)?);
                }
            }
            SessionsCommand::Show { id } => {
                println!("{}", serde_json::to_string_pretty(&db::show_session(&pool, id).await?)?);
            }
            SessionsCommand::History { id } => {
                println!("{}", serde_json::to_string_pretty(&db::history_json(&pool, id).await?)?);
            }
            SessionsCommand::Close { id, reason } => {
                let live_terminated = robdex_agent_runtime::starlark_host::terminate_session_processes_for_close(id);
                db::close_session(&pool, id, &reason, live_terminated).await?;
                println!("closed {id}");
            }
            SessionsCommand::Archive { id } => {
                db::archive_session(&pool, id).await?;
                println!("archived {id}");
            }
            SessionsCommand::Fork { id, at_turn } => {
                let forked = db::fork_session(&pool, id, at_turn).await?;
                println!("{forked}");
            }
        },
        Command::Send { session, message } => {
            runtime::send(&pool, session, &message).await?;
        }
        Command::Compact { session, through_turn } => {
            let budget = compaction::CompactionBudget::from_env();
            let checkpoint = if let Some(through_turn) = through_turn {
                compaction::compact_session_through_turn(&pool, session, through_turn, budget).await?
            } else {
                compaction::compact_session_through_latest_completed_turn(&pool, session, budget).await?
            };
            println!("{}", serde_json::to_string_pretty(&checkpoint)?);
        }
        Command::Events { session } => {
            db::print_events(&pool, session).await?;
        }
        Command::Roles { command } => {
            let registry = RoleRegistry::default_for_workspace()?;
            match command {
                RolesCommand::Create { manifest } => {
                    let imported = registry.load_for_import(&manifest)?;
                    if db::role_exists(&pool, &imported.snapshot.id).await? {
                        anyhow::bail!("role create requires a new role id; role already exists: {}", imported.snapshot.id);
                    }
                    routing::validate_manifest_against_db(&pool, &imported.manifest).await?;
                    command_registry::validate_policy_actions_exist(&pool, imported.snapshot.policy.keys().cloned()).await?;
                    db::import_role_version_with_actor(&pool, &imported, "role-admin-cli").await?;
                    db::append_admin_event(&pool, "role", Some(imported.snapshot.role_version_id), "role.created", Some("active"), serde_json::json!({"roleId": imported.snapshot.id, "version": imported.snapshot.version, "roleVersionId": imported.snapshot.role_version_id})).await?;
                    println!(
                        "role-version {} {} {}",
                        imported.snapshot.id, imported.snapshot.version, imported.snapshot.role_version_id
                    );
                }
                RolesCommand::Update { manifest } => {
                    let imported = registry.load_for_import(&manifest)?;
                    if !db::role_exists(&pool, &imported.snapshot.id).await? {
                        anyhow::bail!("role update requires an existing role id; role does not exist: {}", imported.snapshot.id);
                    }
                    routing::validate_manifest_against_db(&pool, &imported.manifest).await?;
                    command_registry::validate_policy_actions_exist(&pool, imported.snapshot.policy.keys().cloned()).await?;
                    db::import_role_version_with_actor(&pool, &imported, "role-admin-cli").await?;
                    db::append_admin_event(&pool, "role", Some(imported.snapshot.role_version_id), "role.updated", Some("active"), serde_json::json!({"roleId": imported.snapshot.id, "version": imported.snapshot.version, "roleVersionId": imported.snapshot.role_version_id})).await?;
                    println!(
                        "role-version {} {} {}",
                        imported.snapshot.id, imported.snapshot.version, imported.snapshot.role_version_id
                    );
                }
                RolesCommand::Import { manifest } => {
                    let imported = registry.load_for_import(&manifest)?;
                    routing::validate_manifest_against_db(&pool, &imported.manifest).await?;
                    command_registry::validate_policy_actions_exist(&pool, imported.snapshot.policy.keys().cloned()).await?;
                    db::import_role_version_with_actor(&pool, &imported, "role-admin-cli").await?;
                    println!(
                        "role-version {} {} {}",
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
                    for role in db::list_role_records(&pool).await? {
                        println!("{}", serde_json::to_string(&role)?);
                    }
                }
                RolesCommand::Validate { manifest } => {
                    if let Some(path) = manifest {
                        let mut packet = registry.validation_packet_for_path(&path);
                        if packet.valid {
                            match registry.load_for_import(&path) {
                                Ok(imported) => {
                                    if let Err(error) = routing::validate_manifest_against_db(&pool, &imported.manifest).await {
                                        packet.valid = false;
                                        packet.errors.push(error.to_string());
                                    }
                                    if let Err(error) = command_registry::validate_policy_actions_exist(&pool, imported.snapshot.policy.keys().cloned()).await {
                                        packet.valid = false;
                                        packet.errors.push(error.to_string());
                                    }
                                }
                                Err(error) => {
                                    packet.valid = false;
                                    packet.errors.push(error.to_string());
                                }
                            }
                        }
                        println!("{}", serde_json::to_string_pretty(&packet)?);
                        if !packet.valid {
                            db::append_admin_event(&pool, "role", None, "role.validationFailed", Some("failed"), serde_json::json!({"path": path, "packet": packet})).await?;
                            anyhow::bail!("role manifest validation failed");
                        }
                        let imported = registry.load_for_import(&path)?;
                        db::append_admin_event(&pool, "role", None, "role.validationSucceeded", Some("success"), serde_json::json!({"path": path, "roleId": imported.snapshot.id, "version": imported.snapshot.version})).await?;
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
                        println!("{}", serde_json::to_string_pretty(&serde_json::json!({"valid": true, "count": imports.len()}))?);
                        db::append_admin_event(&pool, "role", None, "role.validationSucceeded", Some("success"), serde_json::json!({"count": imports.len()})).await?;
                    }
                }
                RolesCommand::Show { id } => {
                    let role = db::current_role_snapshot(&pool, &id).await?;
                    println!("{}", serde_json::to_string_pretty(&role)?);
                }
                RolesCommand::Versions { id } => {
                    println!("{}", serde_json::to_string_pretty(&db::role_versions(&pool, &id).await?)?);
                }
                RolesCommand::Version { id } => {
                    println!("{}", serde_json::to_string_pretty(&db::role_version_snapshot(&pool, id).await?)?);
                }
                RolesCommand::Activate { id, version_id } => {
                    let snapshot = db::role_version_snapshot(&pool, version_id).await?;
                    routing::validate_snapshot_routing_against_db(&pool, &snapshot).await?;
                    command_registry::validate_policy_actions_exist(&pool, snapshot.policy.keys().cloned()).await?;
                    db::activate_role_version(&pool, &id, version_id).await?;
                    println!("activated {id} {version_id}");
                }
                RolesCommand::Archive { id } => {
                    db::archive_role(&pool, &id).await?;
                    println!("archived {id}");
                }
                RolesCommand::Unarchive { id } => {
                    db::unarchive_role(&pool, &id).await?;
                    println!("unarchived {id}");
                }
                RolesCommand::Export { id, out } => {
                    let export = db::export_role(&pool, &id).await?;
                    let text = serde_json::to_string_pretty(&export)?;
                    if let Some(path) = out {
                        std::fs::write(&path, text)?;
                        println!("{}", path.display());
                    } else {
                        println!("{text}");
                    }
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
            CommandRegistryCommand::SeedRequests { session, mode } => {
                for id in command_registry::create_seed_import_requests(&pool, session, &mode).await? {
                    println!("{id}");
                }
            }
            CommandRegistryCommand::Requests { command } => match command {
                CommandRegistryRequestCommand::List => {
                    println!("{}", serde_json::to_string_pretty(&command_registry::list_requests(&pool).await?)?);
                }
                CommandRegistryRequestCommand::Show { id } => {
                    println!("{}", serde_json::to_string_pretty(&command_registry::show_request(&pool, id).await?)?);
                }
                CommandRegistryRequestCommand::Review { id } => {
                    println!("{}", serde_json::to_string_pretty(&command_registry::review_request(&pool, id).await?)?);
                }
                CommandRegistryRequestCommand::FinalTemplate { id, out } => {
                    let template = command_registry::final_template(&pool, id).await?;
                    let text = serde_json::to_string_pretty(&template)?;
                    if let Some(path) = out {
                        std::fs::write(path, text)?;
                    } else {
                        println!("{text}");
                    }
                }
                CommandRegistryRequestCommand::PreviewDecision { id, status, final_scope, final_project, final_policy, final_command_file } => {
                    let scope = final_scope.map(|scope_type| command_registry::RegistryScope { scope_type, project_key: final_project });
                    let policy = final_policy
                        .map(|decision| command_registry::FinalExecutionPolicy { decision, reason: None });
                    let final_command = match final_command_file {
                        Some(path) => {
                            let raw = std::fs::read_to_string(path)?;
                            let value: serde_json::Value = serde_json::from_str(&raw)?;
                            if let Some(command) = value.get("command").cloned() {
                                Some(serde_json::from_value(command)?)
                            } else {
                                Some(serde_json::from_value(value)?)
                            }
                        }
                        None => None,
                    };
                    println!("{}", serde_json::to_string_pretty(&command_registry::preview_decision(&pool, id, &status, scope, policy, final_command).await?)?);
                }
                CommandRegistryRequestCommand::Decide { session, id, status, final_scope, final_project, final_policy, final_command_file } => {
                    let scope = final_scope.map(|scope_type| command_registry::RegistryScope { scope_type, project_key: final_project });
                    let policy = final_policy
                        .map(|decision| command_registry::FinalExecutionPolicy { decision, reason: None });
                    let final_command = match final_command_file {
                        Some(path) => {
                            let raw = std::fs::read_to_string(path)?;
                            let value: serde_json::Value = serde_json::from_str(&raw)?;
                            if let Some(command) = value.get("command").cloned() {
                                Some(serde_json::from_value(command)?)
                            } else {
                                Some(serde_json::from_value(value)?)
                            }
                        }
                        None => None,
                    };
                    command_registry::decide_request(&pool, session, id, &status, scope, policy, final_command).await?;
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

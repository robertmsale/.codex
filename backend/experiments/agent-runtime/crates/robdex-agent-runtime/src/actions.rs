use anyhow::{Result, bail};

pub const ACTIVE_ACTIONS: &[&str] = &[
    "tool.execute_code",
    "tool.request_command_registry_change",
    "fs.read",
    "fs.write",
    "patch.apply",
    "workflow_memory.search",
    "workflow_memory.remember.project",
    "workflow_memory.remember.global",
    "workflow_memory.feedback",
    "command_registry.request",
    "command_registry.decide",
    "command_registry.apply",
    "project_runtime.request_change",
];
pub const RESERVED_ACTIONS: &[&str] = &[
    "agent.spawn.<role>",
    "agent.archive",
    "requirements.set.self",
    "requirements.set.other",
    "requirements.change.active",
    "message.send",
    "message.route",
];

pub fn is_known_action(action: &str) -> bool {
    ACTIVE_ACTIONS.contains(&action) || RESERVED_ACTIONS.contains(&action)
}

pub fn is_active_action(action: &str) -> bool {
    ACTIVE_ACTIONS.contains(&action) || crate::command_registry::is_registry_command_action(action)
}

pub fn validate_known_action(action: &str) -> Result<()> {
    if is_known_action(action) {
        Ok(())
    } else {
        bail!("unknown action in role manifest: {action}")
    }
}

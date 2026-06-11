use anyhow::{Result, bail};

pub const ACTIVE_ACTIONS: &[&str] = &["tool.execute_code", "fs.read", "cmd.rg.run"];
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
    ACTIVE_ACTIONS.contains(&action)
}

pub fn validate_known_action(action: &str) -> Result<()> {
    if is_known_action(action) {
        Ok(())
    } else {
        bail!("unknown action in role manifest: {action}")
    }
}

use anyhow::{Result, bail};

pub const ACTIVE_ACTIONS: &[&str] = &[
    "tool.execute_code",
    "tool.request_command_registry_change",
    "fs.read",
    "fs.write",
    "file.head",
    "file.tail",
    "file.read_lines",
    "file.line_count",
    "file.search",
    "file.replace_exact",
    "tree.list",
    "tree.find",
    "patch.apply",
    "git.status",
    "git.diff",
    "git.restore",
    "git.add",
    "git.commit",
    "git.inspect_worker_branch",
    "git.rebase_worker_branch",
    "git.fast_forward_local_main",
    "git.cleanup_integrated_worktree",
    "server.start",
    "server.status",
    "server.logs",
    "server.stop",
    "image.capture_from_file",
    "image.describe",
    "tooling.request",
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

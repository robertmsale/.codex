use robdex_protocol::{AppSnapshot, BridgeConnectionStatus, ProjectSummary, ThreadSummary};

#[derive(Debug, Clone, Default)]
pub struct ClientState {
    pub connection_status: Option<BridgeConnectionStatus>,
    pub selected_project_id: Option<String>,
    pub selected_thread_id: Option<String>,
    pub projects: Vec<ProjectSummary>,
    pub threads: Vec<ThreadSummary>,
}

impl ClientState {
    pub fn apply_snapshot(&mut self, snapshot: AppSnapshot) {
        self.connection_status = Some(snapshot.connection_status);
        self.selected_project_id = snapshot.selected_project_id;
        self.selected_thread_id = snapshot.selected_thread_id;
        self.projects = snapshot.projects;
        self.threads = snapshot.threads;
    }
}

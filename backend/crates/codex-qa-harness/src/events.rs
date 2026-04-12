use std::sync::Arc;

use serde::Serialize;
use tokio::sync::broadcast;

use crate::{models::SlotRuntimeState, state::iso_timestamp};

#[derive(Debug, Clone, Serialize)]
pub struct HarnessEvent {
    pub kind: String,
    pub at: String,
    pub project_id: String,
    pub device_key: String,
    pub state: SlotRuntimeState,
}

#[derive(Clone)]
pub struct EventBus {
    sender: Arc<broadcast::Sender<HarnessEvent>>,
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new(256)
    }
}

impl EventBus {
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self {
            sender: Arc::new(sender),
        }
    }

    pub fn publish_slot_state(&self, kind: impl Into<String>, state: &SlotRuntimeState) {
        let _ = self.sender.send(HarnessEvent {
            kind: kind.into(),
            at: iso_timestamp(),
            project_id: state.project_id.clone(),
            device_key: state.device_key.clone(),
            state: state.clone(),
        });
    }

    pub fn subscribe(&self) -> broadcast::Receiver<HarnessEvent> {
        self.sender.subscribe()
    }
}

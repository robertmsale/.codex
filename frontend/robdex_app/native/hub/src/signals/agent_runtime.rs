use rinf::{DartSignal, RustSignal};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, DartSignal)]
pub struct AgentRuntimeRequestSignal {
    pub request_id: String,
    pub packet_json: String,
}

#[derive(Serialize, RustSignal)]
pub struct AgentRuntimeOutputSignal {
    pub request_id: String,
    pub output_json: String,
}

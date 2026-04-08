use anyhow::Result;
use robdex_client_core::BridgeEndpoint;
use robdex_protocol::AppSnapshot;

pub async fn fetch_local_snapshot() -> Result<AppSnapshot> {
    BridgeEndpoint::localhost().fetch_snapshot().await
}

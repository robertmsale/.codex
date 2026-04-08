use anyhow::Result;
use reqwest::Url;
use robdex_protocol::{AppSnapshot, BridgeCommandEnvelope};

#[derive(Debug, Clone)]
pub struct BridgeEndpoint {
    pub http_base: Url,
    pub ws_url: Url,
}

impl BridgeEndpoint {
    pub fn localhost() -> Self {
        Self::new("127.0.0.1", 42080)
    }

    pub fn new(host: &str, port: u16) -> Self {
        Self {
            http_base: Url::parse(&format!("http://{host}:{port}")).expect("valid http url"),
            ws_url: Url::parse(&format!("ws://{host}:{port}/ws")).expect("valid websocket url"),
        }
    }

    pub fn workbench_bootstrap_url(&self) -> Result<Url> {
        Ok(self.http_base.join("/workbench/bootstrap")?)
    }

    pub fn app_state_url(&self) -> Result<Url> {
        Ok(self.http_base.join("/state/app")?)
    }

    pub fn models_url(&self) -> Result<Url> {
        Ok(self.http_base.join("/models")?)
    }

    pub fn workbench_ws_url(&self) -> Result<Url> {
        let host = self
            .http_base
            .host_str()
            .ok_or_else(|| anyhow::anyhow!("bridge endpoint missing host"))?;
        let port = self
            .http_base
            .port_or_known_default()
            .ok_or_else(|| anyhow::anyhow!("bridge endpoint missing port"))?;
        Ok(Url::parse(&format!("ws://{host}:{port}/workbench/ws"))?)
    }

    pub async fn fetch_snapshot(&self) -> Result<AppSnapshot> {
        let url = self.http_base.join("/state/snapshot")?;
        Ok(reqwest::get(url).await?.json::<AppSnapshot>().await?)
    }

    pub fn command(&self, name: impl Into<String>, payload: serde_json::Value) -> BridgeCommandEnvelope {
        BridgeCommandEnvelope {
            id: next_command_id(),
            name: name.into(),
            payload,
        }
    }
}


fn next_command_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_nanos())
        .unwrap_or_default();
    format!("cmd-{nanos}")
}

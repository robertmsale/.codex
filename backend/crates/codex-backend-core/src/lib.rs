use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use anyhow::{Result, anyhow};
use axum::{Json, Router, routing::get};
use clap::Parser;
use serde::Serialize;
use tracing_subscriber::{EnvFilter, fmt};

#[derive(Debug, Clone, Parser)]
pub struct HttpArgs {
    #[arg(long, env = "CODEX_BACKEND_BIND", default_value = "127.0.0.1")]
    pub host: IpAddr,

    #[arg(long, env = "CODEX_BACKEND_PORT", default_value_t = 0)]
    pub port: u16,
}

impl Default for HttpArgs {
    fn default() -> Self {
        Self {
            host: IpAddr::V4(Ipv4Addr::LOCALHOST),
            port: 0,
        }
    }
}

impl HttpArgs {
    pub fn socket_addr(&self) -> SocketAddr {
        SocketAddr::from((self.host, self.port))
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct HealthResponse {
    pub ok: bool,
    pub service: &'static str,
    pub status: &'static str,
    pub phase: &'static str,
}

pub fn init_tracing(service_name: &str) -> Result<()> {
    let filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new(format!("{service_name}=info,tower_http=info")))?;

    fmt()
        .with_env_filter(filter)
        .with_target(false)
        .try_init()
        .map_err(|error| anyhow!(error.to_string()))?;
    Ok(())
}

pub fn scaffold_router(service_name: &'static str) -> Router {
    Router::new().route(
        "/healthz",
        get(move || async move {
            Json(HealthResponse {
                ok: true,
                service: service_name,
                status: "scaffold",
                phase: "not-implemented",
            })
        }),
    )
}

use std::{
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use clap::Parser;
use codex_backend_core::init_tracing;
use codex_qa_harness::{HarnessRuntime, build_router_with_service_name, load_harness_config};
use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;
use tracing::info;

const DEFAULT_CONFIG_DIR: &str = "/Users/robertsale/.codex/backend/config/flutter-sim/projects";
const DEFAULT_STATE_ROOT: &str = "/Users/robertsale/.codex/backend/state/flutter-sim";

#[derive(Debug, Clone, Parser)]
struct Args {
    #[arg(long, env = "CODEX_FLUTTER_SIM_BIND", default_value = "127.0.0.1")]
    host: std::net::IpAddr,

    #[arg(long, env = "CODEX_FLUTTER_SIM_PORT", default_value_t = 8767)]
    port: u16,

    #[arg(long, env = "CODEX_FLUTTER_SIM_CONFIG_DIR", default_value = DEFAULT_CONFIG_DIR)]
    config_dir: PathBuf,

    #[arg(long, env = "CODEX_FLUTTER_SIM_STATE_ROOT", default_value = DEFAULT_STATE_ROOT)]
    state_root: PathBuf,
}

impl Args {
    fn config_dir(&self) -> &Path {
        &self.config_dir
    }

    fn state_root(&self) -> &Path {
        &self.state_root
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    init_tracing("codex_flutter_sim_http")?;

    let config = load_harness_config(args.config_dir())
        .with_context(|| format!("load flutter sim broker config from {}", args.config_dir().display()))?;
    let runtime = HarnessRuntime::from_config(config, args.state_root().to_path_buf())
        .with_context(|| format!("initialize flutter sim broker state in {}", args.state_root().display()))?;

    let listener = TcpListener::bind((args.host, args.port)).await?;
    info!(
        "codex-flutter-sim-http listening on {} with {} configured project(s); config dir {}; state root {}",
        listener.local_addr()?,
        runtime.project_count(),
        args.config_dir().display(),
        runtime.state_root().display()
    );

    let app = build_router_with_service_name(runtime, "codex-flutter-sim-http")
        .layer(TraceLayer::new_for_http());
    axum::serve(listener, app).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_default_to_backend_local_config_and_state_roots() {
        let args = Args::parse_from(["codex-flutter-sim-http"]);
        assert_eq!(args.config_dir(), Path::new(DEFAULT_CONFIG_DIR));
        assert_eq!(args.state_root(), Path::new(DEFAULT_STATE_ROOT));
    }

    #[test]
    fn args_accept_explicit_config_and_state_roots() {
        let args = Args::parse_from([
            "codex-flutter-sim-http",
            "--config-dir",
            "/tmp/flutter-sim-config",
            "--state-root",
            "/tmp/flutter-sim-state",
        ]);
        assert_eq!(args.config_dir(), Path::new("/tmp/flutter-sim-config"));
        assert_eq!(args.state_root(), Path::new("/tmp/flutter-sim-state"));
    }
}

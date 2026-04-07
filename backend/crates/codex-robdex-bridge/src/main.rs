use anyhow::Result;
use clap::Parser;
use codex_backend_core::init_tracing;
use codex_robdex_bridge::{BridgeArgs, BridgeRuntime, build_router};
use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    let args = BridgeArgs::parse();
    init_tracing("codex_robdex_bridge")?;

    let runtime = BridgeRuntime::new(args.settings()?).await?;
    let _transport_task = runtime.spawn_transport();
    let listener = TcpListener::bind(runtime.settings().http.socket_addr()).await?;
    info!(
        "codex-robdex-bridge listening on {} for project {}",
        listener.local_addr()?,
        runtime.settings().project_path.display()
    );

    let app = build_router(runtime).layer(TraceLayer::new_for_http());
    axum::serve(listener, app).await?;
    Ok(())
}

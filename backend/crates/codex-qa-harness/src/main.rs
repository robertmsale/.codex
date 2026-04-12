use anyhow::Result;
use clap::Parser;
use codex_backend_core::init_tracing;
use codex_qa_harness::{HarnessArgs, HarnessRuntime, build_router};
use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    let args = HarnessArgs::parse();
    init_tracing("codex_qa_harness")?;

    let runtime = HarnessRuntime::load(&args)?;
    let listener = TcpListener::bind((args.host, args.port)).await?;
    info!(
        "codex-qa-harness listening on {} with {} configured project(s); state root {}",
        listener.local_addr()?,
        runtime.project_count(),
        runtime.state_root().display()
    );

    let app = build_router(runtime).layer(TraceLayer::new_for_http());
    axum::serve(listener, app).await?;
    Ok(())
}

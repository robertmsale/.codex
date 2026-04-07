use anyhow::Result;
use axum::Router;
use clap::Parser;
use codex_backend_core::{HttpArgs, init_tracing, scaffold_router};
use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;
use tracing::info;

#[derive(Debug, Parser)]
struct Args {
    #[arg(long, env = "PARALLELS_SYNC_FLUTTER_DRIVE_BIND", default_value = "127.0.0.1")]
    host: std::net::IpAddr,

    #[arg(long, env = "PARALLELS_SYNC_FLUTTER_DRIVE_PORT", default_value_t = 8768)]
    port: u16,
}

impl Args {
    fn http(&self) -> HttpArgs {
        HttpArgs {
            host: self.host,
            port: self.port,
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    init_tracing("codex_flutter_drive_http")?;

    let app = Router::new()
        .merge(scaffold_router("codex-flutter-drive-http"))
        .layer(TraceLayer::new_for_http());

    let listener = TcpListener::bind(args.http().socket_addr()).await?;
    info!(
        "codex-flutter-drive-http scaffold listening on {}",
        listener.local_addr()?
    );
    axum::serve(listener, app).await?;
    Ok(())
}


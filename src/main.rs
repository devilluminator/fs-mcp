use std::sync::Arc;

use fs_mcp::{config::Config, server::FsServer};
use rmcp::transport::streamable_http_server::{
    StreamableHttpService, session::local::LocalSessionManager,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt().with_writer(std::io::stderr).init();

    let cfg = Arc::new(Config::from_env()?);
    let factory_cfg = cfg.clone();

    let service = StreamableHttpService::new(
        move || Ok(FsServer::new(factory_cfg.clone())),
        LocalSessionManager::default().into(),
        Default::default(),
    );

    let app = axum::Router::new().nest_service("/mcp", service);
    let listener = tokio::net::TcpListener::bind(cfg.addr).await?;
    match &cfg.jail {
        Some(j) => tracing::info!("fs-mcp jailed to {} at http://{}/mcp", j.display(), cfg.addr),
        None => tracing::info!("fs-mcp unrestricted at http://{}/mcp", cfg.addr),
    }

    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}

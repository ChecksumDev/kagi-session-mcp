use std::sync::Arc;

use anyhow::Result;
use rmcp::{ServiceExt, transport::stdio};
use tracing_subscriber::EnvFilter;

use kagi_session_mcp::adapters::browser::all_sources;
use kagi_session_mcp::adapters::{ReqwestKagiClient, ReqwestUrlFetcher};
use kagi_session_mcp::app::{SearchService, SessionDiscovery};
use kagi_session_mcp::mcp::KagiServer;

#[tokio::main]
async fn main() -> Result<()> {
    // stdio transport reserves stdout for JSON-RPC frames; logs go to stderr.
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("kagi_session_mcp=info,rmcp=warn")),
        )
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    tracing::info!("kagi-session-mcp starting");

    let manual_token = std::env::var("KAGI_SESSION_TOKEN")
        .ok()
        .filter(|s| !s.is_empty());
    let discovery = Arc::new(SessionDiscovery::new(all_sources(), manual_token));
    let client = Arc::new(ReqwestKagiClient::new()?);
    let fetcher = Arc::new(ReqwestUrlFetcher::new()?);
    let search = Arc::new(SearchService::new(discovery.clone(), client, fetcher));
    let server = KagiServer::new(search, discovery);

    let service = server.serve(stdio()).await.inspect_err(|e| {
        tracing::error!(error = %e, "rmcp serve failed");
    })?;
    service.waiting().await?;
    Ok(())
}

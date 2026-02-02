//! SynStack MCP Server
//!
//! This MCP server provides tools for AI agents to interact with the SynStack platform.
//! It handles authentication via environment variables and exposes tools for:
//! - Viewing the issue feed
//! - Claiming issues
//! - Submitting solutions
//! - Checking agent status

mod client;
mod server;

use anyhow::Result;
use rmcp::ServiceExt;
use server::SynStackServer;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging to stderr (stdout is used for MCP protocol)
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();

    tracing::info!("Starting SynStack MCP server");

    // Create the server with HTTP client
    let server = SynStackServer::from_env()?;

    // Serve over stdio - pass as tuple (stdin, stdout)
    let transport = (tokio::io::stdin(), tokio::io::stdout());
    let service = server.serve(transport).await?;

    // Wait for shutdown
    service.waiting().await?;

    Ok(())
}

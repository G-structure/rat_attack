use std::env;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use ct_bridge::{init_logging, serve, BridgeConfig, BridgeError};

#[tokio::main]
async fn main() -> Result<(), BridgeError> {
    // Initialize logging first
    if let Err(e) = init_logging() {
        eprintln!("Failed to initialize logging: {e}");
        std::process::exit(1);
    }

    let args: Vec<String> = env::args().collect();

    let agent_type = parse_agent_arg(&args);
    let bind_addr = parse_bind_addr(&args);
    let project_root = parse_project_root(&args);

    tracing::info!("CT-BRIDGE starting...");
    tracing::info!(agent_type = %agent_type, "Agent type configured");
    tracing::info!(bind_addr = %bind_addr, "Bind address configured");
    tracing::info!(project_root = %project_root.display(), "Project root configured");

    let config = BridgeConfig {
        bind_addr,
        allowed_origins: vec![
            "http://localhost:5173".to_string(),
            "http://127.0.0.1:5173".to_string(),
            "http://localhost:5174".to_string(),
            "http://127.0.0.1:5174".to_string(),
            "http://localhost:5175".to_string(),
            "http://127.0.0.1:5175".to_string(),
        ],
        expected_subprotocol: "acp.jsonrpc.v1".to_string(),
        bridge_id: "ct-bridge-mvp".to_string(),
        project_root: project_root.clone(),
    };

    let transport = create_agent_transport(agent_type, project_root).await?;

    let handle = serve(config, transport).await?;
    tracing::info!(addr = %handle.local_addr(), "Bridge server listening");

    tokio::signal::ctrl_c()
        .await
        .expect("Failed to listen for Ctrl+C");
    tracing::info!("Shutdown signal received, gracefully shutting down...");

    let _ = handle.shutdown().await;
    tracing::info!("Shutdown complete");
    Ok(())
}

fn parse_agent_arg(args: &[String]) -> &str {
    for i in 0..args.len() {
        if args[i] == "--agent" && i + 1 < args.len() {
            tracing::debug!(agent_type = %args[i + 1], "Agent type specified via command line");
            return &args[i + 1];
        }
    }
    tracing::debug!("No agent type specified, defaulting to 'dummy'");
    "dummy" // Default to dummy agent
}

fn parse_bind_addr(args: &[String]) -> SocketAddr {
    for i in 0..args.len() {
        if args[i] == "--bind" && i + 1 < args.len() {
            if let Ok(addr) = args[i + 1].parse() {
                tracing::debug!(bind_addr = %addr, "Bind address specified via command line");
                return addr;
            } else {
                tracing::warn!(invalid_addr = %args[i + 1], "Invalid bind address provided, using default");
            }
        }
    }
    let default_addr = "127.0.0.1:8137".parse().unwrap();
    tracing::debug!(bind_addr = %default_addr, "Using default bind address");
    default_addr
}

fn parse_project_root(args: &[String]) -> PathBuf {
    for i in 0..args.len() {
        if args[i] == "--project-root" && i + 1 < args.len() {
            let path = PathBuf::from(&args[i + 1]);
            tracing::debug!(project_root = %path.display(), "Project root specified via command line");
            return path;
        }
    }
    let default_root = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    tracing::debug!(project_root = %default_root.display(), "Using current directory as project root");
    default_root
}

async fn create_agent_transport(
    agent_type: &str,
    project_root: PathBuf,
) -> Result<Arc<dyn ct_bridge::AgentTransport>, BridgeError> {
    tracing::info!(agent_type = %agent_type, "Creating agent transport");

    match agent_type {
        "dummy" => {
            tracing::debug!("Initializing dummy agent transport");
            let transport = ct_bridge::DummyAgentTransport::new(project_root);
            Ok(Arc::new(transport))
        }
        "claude-code-acp" => {
            let command = ".external_agents/claude-code-acp/node_modules/.bin/claude-code-acp";
            tracing::debug!(command = %command, "Initializing Claude Code ACP transport");
            let transport =
                ct_bridge::StdioAgentTransport::new(command, Vec::new(), project_root).await?;
            tracing::info!("Claude Code ACP transport initialized successfully");
            Ok(Arc::new(transport))
        }
        _ => {
            tracing::error!(
                agent_type = %agent_type,
                supported_types = "dummy, claude-code-acp",
                "Unknown agent type specified"
            );
            std::process::exit(1);
        }
    }
}

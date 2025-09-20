//! CT-BRIDGE: Agent Client Protocol WebSocket Bridge
//!
//! This library provides a WebSocket bridge server that implements the Agent Client Protocol (ACP)
//! over WebSocket connections. It enables web applications to communicate with ACP-compatible agents
//! through a standardized JSON-RPC interface.
//!
//! # Architecture
//!
//! The bridge consists of several key components:
//! - **Bridge Server**: WebSocket server that handles client connections and routing
//! - **Protocol Handler**: JSON-RPC request processing and ACP method dispatch
//! - **Transport Layer**: Abstraction for communicating with different agent implementations
//! - **File System**: Sandboxed file operations with permission gating
//! - **Authentication**: CLI-based authentication and login URL extraction
//! - **Permissions**: Permission caching and decision management
//!
//! # Usage
//!
//! ```rust,no_run
//! use ct_bridge::{BridgeConfig, serve, AgentTransport, BridgeError};
//! use std::sync::Arc;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), BridgeError> {
//!     let config = BridgeConfig {
//!         bind_addr: "127.0.0.1:8080".parse().unwrap(),
//!         allowed_origins: vec!["https://example.com".to_string()],
//!         expected_subprotocol: "agent-client-protocol".to_string(),
//!         bridge_id: "my-bridge".to_string(),
//!     };
//!
//!     // Your agent transport implementation would go here
//!     // let transport: Arc<dyn AgentTransport> = ...;
//!
//!     // let handle = serve(config, transport).await?;
//!     // println!("Bridge listening on {}", handle.local_addr());
//!     Ok(())
//! }
//! ```

// Core modules
pub mod bridge;
pub mod transport;
pub mod protocol;

// Feature modules
pub mod filesystem;
pub mod auth;
pub mod permissions;

// Re-exports for public API
pub use bridge::{BridgeConfig, BridgeHandle, BridgeError, serve};
pub use transport::{AgentTransport, AgentTransportError, NotificationSender, WebSocketNotificationSender};
pub use permissions::{PermissionDecision, PermissionCache};

// Internal re-exports for backward compatibility during refactoring
// These maintain the existing public interface while code is being modularized
pub use protocol::request::handle_websocket_messages;
pub use filesystem::{handle_read_text_file, handle_write_text_file, validate_and_resolve_path};
pub use auth::handle_auth_cli_login;
//! Bridge configuration types
//!
//! This module defines the configuration structures used to set up and manage
//! CT-BRIDGE instances. It includes both public configuration and internal
//! shared state.

use std::{collections::HashMap, net::SocketAddr, path::PathBuf, sync::Arc};
use tokio::sync::Mutex as TokioMutex;

use crate::permissions::PermissionDecision;

/// Configuration for a CT-BRIDGE instance
///
/// This struct contains all the parameters needed to start a bridge server,
/// including network settings, security policies, and identification.
///
/// # Spec Compliance
/// - `bridge_id` implements RAT-LWS-REQ-300 (Bridge Identity)
/// - `allowed_origins` implements RAT-LWS-REQ-001 (Origin validation)
/// - `expected_subprotocol` implements RAT-LWS-REQ-002 (Subprotocol echo)
#[derive(Clone, Debug)]
pub struct BridgeConfig {
    /// Socket address to bind the WebSocket server to
    pub bind_addr: SocketAddr,
    /// List of allowed Origin headers for WebSocket upgrade requests
    pub allowed_origins: Vec<String>,
    /// Expected WebSocket subprotocol (should be "acp.jsonrpc.v1")
    pub expected_subprotocol: String,
    /// Stable, opaque identifier for this bridge instance
    pub bridge_id: String,
    /// Project root path for file operations sandboxing (RAT-LWS-REQ-044)
    pub project_root: PathBuf,
}

/// Internal shared configuration for bridge operations
///
/// This struct contains configuration and state that is shared across
/// all connections to a bridge instance. It includes security policies
/// and permission caching.
pub struct BridgeSharedConfig {
    /// List of allowed Origin headers for WebSocket connections
    pub allowed_origins: Vec<String>,
    /// Expected WebSocket subprotocol token
    pub expected_subprotocol: String,
    /// Stable identifier for this bridge instance (exposed in initialize response)
    pub bridge_id: String,
    /// Project root path for file operations sandboxing (RAT-LWS-REQ-044)
    pub project_root: PathBuf,
    /// Cache for permission decisions to avoid repeated prompts
    pub permission_cache: Arc<TokioMutex<HashMap<(String, String), PermissionDecision>>>,
}

impl BridgeSharedConfig {
    /// Creates a new shared configuration from a public bridge configuration
    pub fn new(config: BridgeConfig) -> Self {
        Self {
            allowed_origins: config.allowed_origins,
            expected_subprotocol: config.expected_subprotocol,
            bridge_id: config.bridge_id,
            project_root: config.project_root,
            permission_cache: Arc::new(TokioMutex::new(HashMap::new())),
        }
    }
}

/// Type alias for session-specific permission cache
/// Keys are (session_id, path) tuples for isolation per RAT-LWS-REQ-300
pub type PermissionCache = Arc<TokioMutex<HashMap<(String, String), PermissionDecision>>>;

//! Dummy agent transport for testing and development
//!
//! This module provides a simple dummy agent implementation that echoes back
//! user messages and simulates basic ACP functionality for testing purposes.

use agent_client_protocol as acp;
use std::{future::Future, path::PathBuf, pin::Pin, sync::Arc};

use crate::transport::{AgentTransport, AgentTransportError, NotificationSender};

/// A dummy agent transport that simulates basic ACP interactions
///
/// This transport provides mock responses for testing the bridge functionality
/// without requiring an actual agent process.
pub struct DummyAgentTransport {
    #[allow(dead_code)]
    project_root: PathBuf,
}

impl DummyAgentTransport {
    /// Creates a new dummy agent transport
    pub fn new(project_root: PathBuf) -> Self {
        Self { project_root }
    }
}

impl AgentTransport for DummyAgentTransport {
    fn initialize(
        &self,
        request: acp::InitializeRequest,
    ) -> Pin<Box<dyn Future<Output = Result<acp::InitializeResponse, AgentTransportError>> + Send>>
    {
        let _request = request; // Acknowledge the parameter

        Box::pin(async move {
            Ok(acp::InitializeResponse {
                protocol_version: acp::VERSION,
                agent_capabilities: acp::AgentCapabilities {
                    load_session: true,
                    ..Default::default()
                },
                auth_methods: Vec::new(),
                meta: Some(serde_json::json!({
                    "bridgeId": "dummy-bridge"
                })),
            })
        })
    }

    fn new_session(
        &self,
        request: acp::NewSessionRequest,
    ) -> Pin<Box<dyn Future<Output = Result<acp::NewSessionResponse, AgentTransportError>> + Send>>
    {
        let _request = request; // Acknowledge the parameter

        Box::pin(async move {
            Ok(acp::NewSessionResponse {
                session_id: acp::SessionId(
                    format!("dummy-session-{}", chrono::Utc::now().timestamp()).into(),
                ),
                modes: None,
                meta: None,
            })
        })
    }

    fn prompt(
        &self,
        request: acp::PromptRequest,
        _notification_sender: Arc<dyn NotificationSender>,
    ) -> Pin<Box<dyn Future<Output = Result<acp::PromptResponse, AgentTransportError>> + Send>>
    {
        let _request = request; // Acknowledge the parameter

        Box::pin(async move {
            // For the dummy agent, just return a simple completion response
            // Session updates and streaming will be handled later once we understand the types better
            Ok(acp::PromptResponse {
                stop_reason: acp::StopReason::EndTurn,
                meta: None,
            })
        })
    }

    fn request_permission(
        &self,
        request: acp::RequestPermissionRequest,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<acp::RequestPermissionResponse, AgentTransportError>> + Send,
        >,
    > {
        let _request = request; // Acknowledge the parameter

        Box::pin(async move {
            // For dummy agent, always allow operations for testing
            Ok(acp::RequestPermissionResponse {
                outcome: acp::RequestPermissionOutcome::Selected {
                    option_id: acp::PermissionOptionId("allow_once".into()),
                },
                meta: None,
            })
        })
    }
}

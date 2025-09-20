//! Agent transport layer
//!
//! This module defines the traits and types for communicating with ACP agents.
//! It handles the abstraction between the bridge and actual agent implementations.

use std::{pin::Pin, future::Future, sync::Arc};
use agent_client_protocol as acp;

pub mod error;
pub mod notification;
pub mod dummy;
pub mod stdio;

pub use error::AgentTransportError;
pub use notification::{NotificationSender, WebSocketNotificationSender};
pub use dummy::DummyAgentTransport;
pub use stdio::StdioAgentTransport;

/// Trait for agent transport implementations
///
/// This trait defines the interface that all agent transports must implement
/// to handle ACP (Agent Client Protocol) communication. It provides methods
/// for initialization, session management, and request processing.
///
/// # Spec Compliance
/// - `initialize` implements RAT-LWS-REQ-020 (initialize method)
/// - `new_session` implements RAT-LWS-REQ-030 (session creation)
/// - `prompt` implements RAT-LWS-REQ-031 (streaming prompt processing)
/// - `request_permission` implements RAT-LWS-REQ-041 (permission gating)
pub trait AgentTransport: Send + Sync + 'static {
    /// Initializes the agent and returns capabilities
    ///
    /// This method is called when a client connects and sends an initialize
    /// request. The agent should return its capabilities and any metadata.
    fn initialize(
        &self,
        request: acp::InitializeRequest,
    ) -> Pin<Box<dyn Future<Output = Result<acp::InitializeResponse, AgentTransportError>> + Send>>;

    /// Creates a new session
    ///
    /// This method creates a new conversation session with the agent.
    /// Sessions are used to maintain conversation context and state.
    fn new_session(
        &self,
        request: acp::NewSessionRequest,
    ) -> Pin<Box<dyn Future<Output = Result<acp::NewSessionResponse, AgentTransportError>> + Send>>;

    /// Processes a prompt and streams responses
    ///
    /// This method sends a prompt to the agent and expects streaming updates
    /// to be sent via the notification sender. The agent should stream
    /// session/update notifications until the response is complete.
    fn prompt(
        &self,
        request: acp::PromptRequest,
        notification_sender: Arc<dyn NotificationSender>,
    ) -> Pin<Box<dyn Future<Output = Result<acp::PromptResponse, AgentTransportError>> + Send>>;

    /// Requests permission for an operation
    ///
    /// This method is called when the agent needs permission to perform
    /// a potentially sensitive operation (like writing files). The transport
    /// should present the permission options to the user and return their choice.
    fn request_permission(
        &self,
        request: acp::RequestPermissionRequest,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<acp::RequestPermissionResponse, AgentTransportError>> + Send,
        >,
    >;
}
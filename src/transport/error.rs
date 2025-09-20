//! Transport layer error types
//!
//! This module defines error types for the agent transport layer, providing
//! structured error handling for ACP communication failures.

use agent_client_protocol as acp;

/// Errors that can occur during agent transport operations
///
/// This enum represents the different types of failures that can happen
/// when communicating with ACP agents through the transport layer.
#[derive(Debug)]
pub enum AgentTransportError {
    /// Error from the ACP protocol layer
    Protocol(acp::Error),
    /// Internal transport error with description
    Internal(String),
    /// Requested functionality is not implemented
    NotImplemented,
    /// Failed to spawn or communicate with agent process
    ProcessSpawn(String),
    /// Communication error with agent
    Communication(String),
}

impl From<acp::Error> for AgentTransportError {
    fn from(value: acp::Error) -> Self {
        AgentTransportError::Protocol(value)
    }
}

impl AgentTransportError {
    /// Converts this transport error into a JSON-RPC error for client responses
    ///
    /// This method ensures that all transport errors are properly formatted
    /// as ACP-compliant JSON-RPC error responses.
    pub fn into_rpc_error(self) -> acp::Error {
        match self {
            AgentTransportError::Protocol(err) => err,
            AgentTransportError::Internal(message) => {
                acp::Error::internal_error().with_data(message)
            }
            AgentTransportError::NotImplemented => {
                acp::Error::internal_error().with_data("agent transport not implemented")
            }
            AgentTransportError::ProcessSpawn(message) => {
                acp::Error::internal_error().with_data(format!("process spawn error: {}", message))
            }
            AgentTransportError::Communication(message) => {
                acp::Error::internal_error().with_data(format!("communication error: {}", message))
            }
        }
    }
}
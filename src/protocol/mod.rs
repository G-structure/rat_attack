//! ACP protocol handling
//!
//! This module handles ACP (Agent Client Protocol) request processing and JSON-RPC messaging
//! over WebSocket connections. It implements the protocol requirements from the RAT spec.

pub mod validation;
pub mod messaging;
pub mod request;

// Re-export commonly used functions
pub use validation::{validate_origin, validate_subprotocol, handshake_error};
pub use messaging::{send_result, send_error, send_result_shared, send_error_shared, send_json};
pub use request::handle_websocket_messages;
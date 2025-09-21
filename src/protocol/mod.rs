//! ACP protocol handling
//!
//! This module handles ACP (Agent Client Protocol) request processing and JSON-RPC messaging
//! over WebSocket connections. It implements the protocol requirements from the RAT spec.

pub mod messaging;
pub mod request;
pub mod validation;

// Re-export commonly used functions
pub use messaging::{send_error, send_error_shared, send_json, send_result, send_result_shared};
pub use request::handle_websocket_messages;
pub use validation::{handshake_error, validate_origin, validate_subprotocol};

//! WebSocket validation utilities
//!
//! This module provides validation functions for WebSocket upgrade requests,
//! including origin checking and subprotocol validation as required by the RAT spec.

use tokio_tungstenite::tungstenite::handshake::server::{
    ErrorResponse, Request, Response as HandshakeResponse,
};
use tokio_tungstenite::tungstenite::http::header::{HeaderValue, ORIGIN, SEC_WEBSOCKET_PROTOCOL};
use tokio_tungstenite::tungstenite::http::{Response as HttpResponse, StatusCode};

/// Validates the Origin header against an allowlist
///
/// This function implements RAT-LWS-REQ-001 by checking that the Origin header
/// matches one of the allowed origins for the bridge instance.
///
/// # Arguments
/// - `request`: The WebSocket upgrade request
/// - `allowed_origins`: List of allowed origin values
///
/// # Returns
/// - `Ok(())` if the origin is allowed
/// - `Err(ErrorResponse)` if the origin is not allowed or missing
#[allow(clippy::result_large_err)]
pub fn validate_origin(request: &Request, allowed_origins: &[String]) -> Result<(), ErrorResponse> {
    let origin = request
        .headers()
        .get(ORIGIN)
        .and_then(|value| value.to_str().ok());

    match origin {
        Some(origin_value)
            if allowed_origins
                .iter()
                .any(|allowed| allowed == origin_value) =>
        {
            Ok(())
        }
        _ => Err(handshake_error(StatusCode::FORBIDDEN, "Origin not allowed")),
    }
}

/// Validates and echoes the WebSocket subprotocol
///
/// This function implements RAT-LWS-REQ-002 by checking that the expected
/// subprotocol is offered by the client and echoing it back in the response.
///
/// # Arguments
/// - `request`: The WebSocket upgrade request
/// - `response`: The WebSocket upgrade response to modify
/// - `expected`: The expected subprotocol (should be "acp.jsonrpc.v1")
///
/// # Returns
/// - `Ok(())` if the subprotocol is valid and was echoed
/// - `Err(ErrorResponse)` if the subprotocol is missing or invalid
#[allow(clippy::result_large_err)]
pub fn validate_subprotocol(
    request: &Request,
    response: &mut HandshakeResponse,
    expected: &str,
) -> Result<(), ErrorResponse> {
    let header = request
        .headers()
        .get(SEC_WEBSOCKET_PROTOCOL)
        .and_then(|value| value.to_str().ok());

    let has_expected = header.map(|value| {
        value
            .split(',')
            .any(|candidate| candidate.trim().eq_ignore_ascii_case(expected))
    });

    if has_expected != Some(true) {
        return Err(handshake_error(
            StatusCode::UPGRADE_REQUIRED,
            "Missing required subprotocol",
        ));
    }

    let header_value = HeaderValue::from_str(expected)
        .map_err(|_| handshake_error(StatusCode::BAD_REQUEST, "Invalid subprotocol"))?;
    response
        .headers_mut()
        .insert(SEC_WEBSOCKET_PROTOCOL, header_value);
    Ok(())
}

/// Creates a standardized WebSocket handshake error response
///
/// This function creates HTTP error responses for WebSocket handshake failures
/// with appropriate status codes and error messages.
///
/// # Arguments
/// - `status`: The HTTP status code for the error
/// - `message`: Human-readable error message
///
/// # Returns
/// An `ErrorResponse` that can be returned from the handshake callback
pub fn handshake_error(status: StatusCode, message: &str) -> ErrorResponse {
    HttpResponse::builder()
        .status(status)
        .body(Some(message.to_owned()))
        .unwrap_or_else(|_| HttpResponse::builder().status(status).body(None).unwrap())
}

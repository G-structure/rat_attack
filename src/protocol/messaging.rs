//! JSON-RPC messaging utilities
//!
//! This module provides utilities for sending and receiving JSON-RPC messages
//! over WebSocket connections, implementing the ACP message format.

use agent_client_protocol as acp;
use futures_util::SinkExt;
use serde_json::Value;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::Mutex as TokioMutex;
use tokio_tungstenite::{tungstenite::protocol::Message, WebSocketStream};

/// Sends a JSON-RPC result response over a WebSocket stream
///
/// This function sends a successful JSON-RPC 2.0 response with the given ID and result.
///
/// # Arguments
/// - `stream`: Mutable reference to the WebSocket stream
/// - `id`: The request ID to include in the response
/// - `result`: The result value to include in the response
///
/// # Returns
/// - `Ok(())` if the message was sent successfully
/// - `Err(tungstenite::Error)` if there was an error sending the message
pub async fn send_result(
    stream: &mut WebSocketStream<TcpStream>,
    id: Value,
    result: Value,
) -> Result<(), tokio_tungstenite::tungstenite::Error> {
    let payload = serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result,
    });
    send_json(stream, payload).await
}

/// Sends a JSON-RPC error response over a WebSocket stream
///
/// This function sends an error JSON-RPC 2.0 response with the given ID and error.
///
/// # Arguments
/// - `stream`: Mutable reference to the WebSocket stream
/// - `id`: The request ID to include in the response
/// - `error`: The ACP error to include in the response
///
/// # Returns
/// - `Ok(())` if the message was sent successfully
/// - `Err(tungstenite::Error)` if there was an error sending the message
pub async fn send_error(
    stream: &mut WebSocketStream<TcpStream>,
    id: Value,
    error: acp::Error,
) -> Result<(), tokio_tungstenite::tungstenite::Error> {
    let payload = serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": error,
    });
    send_json(stream, payload).await
}

/// Sends a JSON-RPC result response over a shared WebSocket stream
///
/// This function is a convenience wrapper around `send_result` that handles
/// locking a shared WebSocket stream before sending the response.
///
/// # Arguments
/// - `stream`: Arc-wrapped Mutex-protected WebSocket stream
/// - `id`: The request ID to include in the response
/// - `result`: The result value to include in the response
///
/// # Returns
/// - `Ok(())` if the message was sent successfully
/// - `Err(tungstenite::Error)` if there was an error sending the message
pub async fn send_result_shared(
    stream: &Arc<TokioMutex<WebSocketStream<TcpStream>>>,
    id: Value,
    result: Value,
) -> Result<(), tokio_tungstenite::tungstenite::Error> {
    let mut guard = stream.lock().await;
    send_result(&mut guard, id, result).await
}

/// Sends a JSON-RPC error response over a shared WebSocket stream
///
/// This function is a convenience wrapper around `send_error` that handles
/// locking a shared WebSocket stream before sending the error.
///
/// # Arguments
/// - `stream`: Arc-wrapped Mutex-protected WebSocket stream
/// - `id`: The request ID to include in the response
/// - `error`: The ACP error to include in the response
///
/// # Returns
/// - `Ok(())` if the message was sent successfully
/// - `Err(tungstenite::Error)` if there was an error sending the message
pub async fn send_error_shared(
    stream: &Arc<TokioMutex<WebSocketStream<TcpStream>>>,
    id: Value,
    error: acp::Error,
) -> Result<(), tokio_tungstenite::tungstenite::Error> {
    let mut guard = stream.lock().await;
    send_error(&mut guard, id, error).await
}

/// Sends a JSON payload over a WebSocket stream
///
/// This function serializes a JSON value to a string and sends it as a Text message
/// over the WebSocket connection.
///
/// # Arguments
/// - `stream`: Mutable reference to the WebSocket stream
/// - `payload`: The JSON value to serialize and send
///
/// # Returns
/// - `Ok(())` if the message was sent successfully
/// - `Err(tungstenite::Error)` if there was an error serializing or sending the message
pub async fn send_json(
    stream: &mut WebSocketStream<TcpStream>,
    payload: Value,
) -> Result<(), tokio_tungstenite::tungstenite::Error> {
    let text = serde_json::to_string(&payload)
        .map_err(|err| tokio_tungstenite::tungstenite::Error::Io(std::io::Error::other(err)))?;
    stream.send(Message::Text(text)).await
}

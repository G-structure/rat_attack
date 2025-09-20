//! Notification sender for agent communication
//!
//! This module provides traits and implementations for sending notifications
//! from the bridge to connected clients during agent operations.

use std::{pin::Pin, future::Future, sync::Arc};
use serde_json::Value;
use tokio::sync::Mutex as TokioMutex;
use tokio_tungstenite::{tungstenite::protocol::Message, WebSocketStream};
use tokio::net::TcpStream;
use futures_util::SinkExt;
use serde_json::json;

use crate::transport::AgentTransportError;

/// Trait for sending notifications during agent operations
///
/// This trait abstracts the mechanism for sending notifications (like streaming
/// updates) from agents back to connected clients. It allows different transport
/// implementations while maintaining a consistent interface.
pub trait NotificationSender: Send + Sync {
    /// Sends a notification with the given method and parameters
    ///
    /// # Arguments
    /// - `method`: The JSON-RPC method name for the notification
    /// - `params`: The parameters to include in the notification
    ///
    /// # Returns
    /// A future that resolves when the notification has been sent, or an error
    /// if the notification could not be delivered.
    fn send_notification(
        &self,
        method: &str,
        params: Value,
    ) -> Pin<Box<dyn Future<Output = Result<(), AgentTransportError>> + Send>>;
}

/// WebSocket-based notification sender
///
/// This implementation sends notifications over a WebSocket connection using
/// JSON-RPC format. It's used by the bridge to forward agent notifications
/// to connected web clients.
pub struct WebSocketNotificationSender {
    stream: Arc<TokioMutex<WebSocketStream<TcpStream>>>,
}

impl WebSocketNotificationSender {
    /// Creates a new WebSocket notification sender
    pub fn new(stream: Arc<TokioMutex<WebSocketStream<TcpStream>>>) -> Self {
        Self { stream }
    }
}

impl NotificationSender for WebSocketNotificationSender {
    fn send_notification(
        &self,
        method: &str,
        params: Value,
    ) -> Pin<Box<dyn Future<Output = Result<(), AgentTransportError>> + Send>> {
        let stream = self.stream.clone();
        let method = method.to_string();
        Box::pin(async move {
            let payload = json!({
                "jsonrpc": "2.0",
                "method": method,
                "params": params,
            });

            let mut guard = stream.lock().await;
            send_json(&mut guard, payload).await.map_err(|_| {
                AgentTransportError::Internal("Failed to send notification".to_string())
            })?;
            Ok(())
        })
    }
}

/// Helper function to send JSON over WebSocket
async fn send_json(
    stream: &mut WebSocketStream<TcpStream>,
    payload: Value,
) -> Result<(), tokio_tungstenite::tungstenite::Error> {
    let text = serde_json::to_string(&payload)
        .map_err(|err| tokio_tungstenite::tungstenite::Error::Io(std::io::Error::other(err)))?;
    stream.send(Message::Text(text)).await
}
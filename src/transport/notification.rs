//! Notification sender for agent communication
//!
//! This module provides traits and implementations for sending notifications
//! from the bridge to connected clients during agent operations.

use futures_util::SinkExt;
use serde_json::json;
use serde_json::Value;
use std::{collections::HashMap, future::Future, pin::Pin, sync::Arc};
use tokio::net::TcpStream;
use tokio::sync::{oneshot, Mutex as TokioMutex};
use tokio_tungstenite::{tungstenite::protocol::Message, WebSocketStream};

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

    /// Sends a request with the given method and parameters, expecting a response
    ///
    /// # Arguments
    /// - `method`: The JSON-RPC method name for the request
    /// - `params`: The parameters to include in the request
    ///
    /// # Returns
    /// A future that resolves to the response value, or an error if the request failed
    fn send_request(
        &self,
        method: &str,
        params: Value,
    ) -> Pin<Box<dyn Future<Output = Result<Value, AgentTransportError>> + Send>>;
}

/// WebSocket-based notification sender
///
/// This implementation sends notifications over a WebSocket connection using
/// JSON-RPC format. It's used by the bridge to forward agent notifications
/// to connected web clients.
pub struct WebSocketNotificationSender {
    stream: Arc<TokioMutex<WebSocketStream<TcpStream>>>,
    pending_requests: Arc<TokioMutex<HashMap<String, oneshot::Sender<Value>>>>,
    request_counter: Arc<TokioMutex<u64>>,
}

impl WebSocketNotificationSender {
    /// Creates a new WebSocket notification sender
    pub fn new(stream: Arc<TokioMutex<WebSocketStream<TcpStream>>>) -> Self {
        Self {
            stream,
            pending_requests: Arc::new(TokioMutex::new(HashMap::new())),
            request_counter: Arc::new(TokioMutex::new(0)),
        }
    }

    /// Handles an incoming response from the WebSocket client
    ///
    /// This method should be called when a JSON-RPC response is received
    /// to complete any pending requests.
    pub async fn handle_response(&self, response: Value) -> Result<(), AgentTransportError> {
        if let Some(id) = response.get("id").and_then(|v| v.as_str()) {
            let mut pending = self.pending_requests.lock().await;
            if let Some(sender) = pending.remove(id) {
                if let Some(result) = response.get("result") {
                    let _ = sender.send(result.clone());
                } else if let Some(error) = response.get("error") {
                    // For now, we'll just drop errors. In production, we might want to handle them differently
                    let _ = sender.send(error.clone());
                }
            }
        }
        Ok(())
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

    fn send_request(
        &self,
        method: &str,
        params: Value,
    ) -> Pin<Box<dyn Future<Output = Result<Value, AgentTransportError>> + Send>> {
        let stream = self.stream.clone();
        let pending_requests = self.pending_requests.clone();
        let request_counter = self.request_counter.clone();
        let method = method.to_string();

        Box::pin(async move {
            // Generate a unique request ID
            let request_id = {
                let mut counter = request_counter.lock().await;
                *counter += 1;
                format!("req_{}", *counter)
            };

            // Create response channel
            let (response_tx, response_rx) = oneshot::channel();

            // Store the pending request
            {
                let mut pending = pending_requests.lock().await;
                pending.insert(request_id.clone(), response_tx);
            }

            // Send the request
            let payload = json!({
                "jsonrpc": "2.0",
                "id": request_id,
                "method": method,
                "params": params,
            });

            {
                let mut guard = stream.lock().await;
                send_json(&mut guard, payload).await.map_err(|_| {
                    AgentTransportError::Internal("Failed to send request".to_string())
                })?;
            }

            // Wait for response
            let response = response_rx
                .await
                .map_err(|_| AgentTransportError::Internal("Request was cancelled".to_string()))?;

            Ok(response)
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

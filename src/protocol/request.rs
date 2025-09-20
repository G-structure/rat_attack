//! ACP request processing and WebSocket message handling
//!
//! This module handles the processing of ACP (Agent Client Protocol) requests
//! received over WebSocket connections. It implements the main message loop
//! and dispatches requests to appropriate handlers.

use std::sync::Arc;
use serde_json::{json, Value};
use tokio::sync::Mutex as TokioMutex;
use tokio_tungstenite::{tungstenite::protocol::Message, WebSocketStream};
use tokio::net::TcpStream;
use futures_util::{SinkExt, StreamExt};
use agent_client_protocol as acp;

use crate::bridge::BridgeSharedConfig;
use crate::transport::{AgentTransport, WebSocketNotificationSender};
use crate::protocol::{send_error, send_result_shared, send_error_shared};

/// Handles WebSocket message processing for a connected client
///
/// This function processes incoming WebSocket messages, parsing JSON-RPC
/// requests and dispatching them to appropriate ACP method handlers.
///
/// # Arguments
/// - `stream`: The WebSocket stream for communication
/// - `shared`: Shared bridge configuration and state
/// - `transport`: Agent transport for handling ACP requests
///
/// # Returns
/// - `Ok(())` if the client disconnected cleanly
/// - `Err(tungstenite::Error)` if there was a WebSocket error
pub async fn handle_websocket_messages(
    stream: WebSocketStream<TcpStream>,
    shared: Arc<BridgeSharedConfig>,
    transport: Arc<dyn AgentTransport>,
) -> Result<(), tokio_tungstenite::tungstenite::Error> {
    tracing::info!("Starting WebSocket message processing loop");
    let stream = Arc::new(TokioMutex::new(stream));
    let mut initialized = false;

    loop {
        let message = {
            let mut stream_guard = stream.lock().await;
            stream_guard.next().await
        };

        match message {
            Some(Ok(Message::Text(text))) => {
                tracing::debug!(message_len = text.len(), "Received text message");
                let value: Value = match serde_json::from_str(&text) {
                    Ok(value) => value,
                    Err(err) => {
                        tracing::warn!(error = %err, "Failed to parse JSON message");
                        let mut stream_guard = stream.lock().await;
                        send_error(&mut stream_guard, Value::Null, acp::Error::parse_error())
                            .await?;
                        continue;
                    }
                };
                process_request(stream.clone(), &shared, &transport, &mut initialized, value)
                    .await?;
            }
            Some(Ok(Message::Binary(bytes))) => {
                tracing::debug!(message_len = bytes.len(), "Received binary message");
                let value: Value = match serde_json::from_slice(&bytes) {
                    Ok(value) => value,
                    Err(err) => {
                        tracing::warn!(error = %err, "Failed to parse JSON from binary message");
                        let mut stream_guard = stream.lock().await;
                        send_error(&mut stream_guard, Value::Null, acp::Error::parse_error())
                            .await?;
                        continue;
                    }
                };
                process_request(stream.clone(), &shared, &transport, &mut initialized, value)
                    .await?;
            }
            Some(Ok(Message::Ping(payload))) => {
                tracing::trace!("Received ping, sending pong");
                let mut stream_guard = stream.lock().await;
                stream_guard.send(Message::Pong(payload)).await?;
            }
            Some(Ok(Message::Pong(_))) => {
                tracing::trace!("Received pong");
            }
            Some(Ok(Message::Close(_))) | None => {
                tracing::info!("WebSocket connection closed");
                break;
            }
            Some(Ok(Message::Frame(_))) => {}
            Some(Err(e)) => return Err(e),
        }
    }

    Ok(())
}

/// Processes a single JSON-RPC request and dispatches to appropriate handler
///
/// This function parses JSON-RPC requests and routes them to the correct
/// ACP method handler based on the method name.
///
/// # Arguments
/// - `stream`: Shared WebSocket stream for sending responses
/// - `shared`: Bridge configuration and shared state
/// - `transport`: Agent transport for handling requests
/// - `initialized`: Whether the connection has been initialized
/// - `value`: The parsed JSON-RPC request
///
/// # Returns
/// - `Ok(())` if the request was processed successfully
/// - `Err(tungstenite::Error)` if there was an error sending the response
async fn process_request(
    stream: Arc<TokioMutex<WebSocketStream<TcpStream>>>,
    shared: &BridgeSharedConfig,
    transport: &Arc<dyn AgentTransport>,
    initialized: &mut bool,
    value: Value,
) -> Result<(), tokio_tungstenite::tungstenite::Error> {
    let id = value.get("id").cloned().unwrap_or(Value::Null);
    let method = value.get("method").and_then(|value| value.as_str());

    let method = match method {
        Some(method) => {
            tracing::debug!(method = %method, request_id = ?id, "Processing ACP request");
            method
        },
        None => {
            tracing::warn!(request_id = ?id, "Request missing method field");
            send_error_shared(&stream, id, acp::Error::invalid_request()).await?;
            return Ok(());
        }
    };

    match method {
        "initialize" => {
            handle_initialize(stream, shared, transport, id, value, initialized).await?;
        }
        "session/new" => {
            if !*initialized {
                tracing::warn!(method = %method, "Method called before initialization");
                let error = acp::Error::method_not_found();
                send_error_shared(&stream, id, error).await?;
                return Ok(());
            }
            handle_session_new(stream, transport, id, value).await?;
        }
        "session/prompt" => {
            if !*initialized {
                tracing::warn!(method = %method, "Method called before initialization");
                let error = acp::Error::method_not_found();
                send_error_shared(&stream, id, error).await?;
                return Ok(());
            }
            handle_session_prompt(stream, transport, id, value).await?;
        }
        "fs/read_text_file" => {
            if !*initialized {
                tracing::warn!(method = %method, "Method called before initialization");
                let error = acp::Error::method_not_found();
                send_error_shared(&stream, id, error).await?;
                return Ok(());
            }
            handle_fs_read_text_file(stream, id, value).await?;
        }
        "fs/write_text_file" => {
            if !*initialized {
                tracing::warn!(method = %method, "Method called before initialization");
                let error = acp::Error::method_not_found();
                send_error_shared(&stream, id, error).await?;
                return Ok(());
            }
            handle_fs_write_text_file(stream, shared, transport, id, value).await?;
        }
        "auth/cli_login" => {
            handle_auth_cli_login(stream, id).await?;
        }
        _ => {
            tracing::warn!(method = %method, "Unknown ACP method");
            let error = acp::Error::method_not_found();
            send_error_shared(&stream, id, error).await?;
        }
    }

    Ok(())
}

/// Handles the initialize ACP method
///
/// Implements RAT-LWS-REQ-020 and RAT-LWS-REQ-300 by processing initialization
/// and injecting the bridge ID into the response metadata.
async fn handle_initialize(
    stream: Arc<TokioMutex<WebSocketStream<TcpStream>>>,
    shared: &BridgeSharedConfig,
    transport: &Arc<dyn AgentTransport>,
    id: Value,
    value: Value,
    initialized: &mut bool,
) -> Result<(), tokio_tungstenite::tungstenite::Error> {
    tracing::info!(request_id = ?id, "Processing initialize request");
    let params = value.get("params").cloned().unwrap_or_else(|| json!({}));
    let request: acp::InitializeRequest = match serde_json::from_value::<acp::InitializeRequest>(params) {
        Ok(request) => {
            tracing::debug!(client_capabilities = ?request.client_capabilities, "Initialize request parsed");
            request
        },
        Err(err) => {
            tracing::warn!(error = %err, "Invalid initialize request parameters");
            send_error_shared(
                &stream,
                id,
                acp::Error::invalid_params().with_data(err.to_string()),
            )
            .await?;
            return Ok(());
        }
    };

    let response = transport.initialize(request).await;
    match response {
        Ok(mut response) => {
            ensure_bridge_meta(&mut response, &shared.bridge_id);
            tracing::info!(bridge_id = %shared.bridge_id, "Bridge initialization successful");
            let result = serde_json::to_value(response)
                .map_err(|err| tokio_tungstenite::tungstenite::Error::Io(std::io::Error::other(err)))?;
            send_result_shared(&stream, id, result).await?;
            *initialized = true;
        }
        Err(err) => {
            tracing::error!(error = ?err, "Bridge initialization failed");
            let error = err.into_rpc_error();
            send_error_shared(&stream, id, error).await?;
        }
    }
    Ok(())
}

/// Handles the session/new ACP method
async fn handle_session_new(
    stream: Arc<TokioMutex<WebSocketStream<TcpStream>>>,
    transport: &Arc<dyn AgentTransport>,
    id: Value,
    value: Value,
) -> Result<(), tokio_tungstenite::tungstenite::Error> {
    let params = value.get("params").cloned().unwrap_or_else(|| json!({}));
    let request: acp::NewSessionRequest = match serde_json::from_value(params) {
        Ok(request) => request,
        Err(err) => {
            send_error_shared(
                &stream,
                id,
                acp::Error::invalid_params().with_data(err.to_string()),
            )
            .await?;
            return Ok(());
        }
    };

    let response = transport.new_session(request).await;
    match response {
        Ok(response) => {
            let result = serde_json::to_value(response)
                .map_err(|err| tokio_tungstenite::tungstenite::Error::Io(std::io::Error::other(err)))?;
            send_result_shared(&stream, id, result).await?;
        }
        Err(err) => {
            let error = err.into_rpc_error();
            send_error_shared(&stream, id, error).await?;
        }
    }
    Ok(())
}

/// Handles the session/prompt ACP method
///
/// Implements RAT-LWS-REQ-031 by forwarding prompt requests and setting up
/// streaming notifications for session updates.
async fn handle_session_prompt(
    stream: Arc<TokioMutex<WebSocketStream<TcpStream>>>,
    transport: &Arc<dyn AgentTransport>,
    id: Value,
    value: Value,
) -> Result<(), tokio_tungstenite::tungstenite::Error> {
    let params = value.get("params").cloned().unwrap_or_else(|| json!({}));

    // Convert from simple { sessionId, prompt } to ACP format
    let session_id = params
        .get("sessionId")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let prompt_text = params
        .get("prompt")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let request = acp::PromptRequest {
        session_id: acp::SessionId(session_id.into()),
        prompt: vec![acp::ContentBlock::from(prompt_text)],
        meta: None,
    };

    let notification_sender = Arc::new(WebSocketNotificationSender::new(stream.clone()));
    let response = transport.prompt(request, notification_sender).await;
    match response {
        Ok(response) => {
            let result = serde_json::to_value(response)
                .map_err(|err| tokio_tungstenite::tungstenite::Error::Io(std::io::Error::other(err)))?;
            send_result_shared(&stream, id, result).await?;
        }
        Err(err) => {
            let error = err.into_rpc_error();
            send_error_shared(&stream, id, error).await?;
        }
    }
    Ok(())
}

/// Handles the fs/read_text_file method
///
/// Implements RAT-LWS-REQ-040 by providing file reading with optional
/// line offset and limit parameters.
async fn handle_fs_read_text_file(
    stream: Arc<TokioMutex<WebSocketStream<TcpStream>>>,
    id: Value,
    value: Value,
) -> Result<(), tokio_tungstenite::tungstenite::Error> {
    use crate::filesystem::handle_read_text_file;

    let params = value.get("params").cloned().unwrap_or_else(|| json!({}));

    // Extract parameters
    let path = match params.get("path").and_then(|v| v.as_str()) {
        Some(path) => path,
        None => {
            send_error_shared(
                &stream,
                id,
                acp::Error::invalid_params().with_data("missing or invalid path parameter"),
            )
            .await?;
            return Ok(());
        }
    };

    let line_offset = params
        .get("line_offset")
        .and_then(|v| v.as_u64())
        .map(|v| v as u32);

    let line_limit = params
        .get("line_limit")
        .and_then(|v| v.as_u64())
        .map(|v| v as u32);

    match handle_read_text_file(path, line_offset, line_limit) {
        Ok(content) => {
            let result = json!({
                "content": content
            });
            send_result_shared(&stream, id, result).await?;
        }
        Err(error) => {
            send_error_shared(&stream, id, error).await?;
        }
    }
    Ok(())
}

/// Handles the fs/write_text_file method
///
/// Implements RAT-LWS-REQ-041 by providing permission-gated file writing
/// with project root sandboxing.
async fn handle_fs_write_text_file(
    stream: Arc<TokioMutex<WebSocketStream<TcpStream>>>,
    shared: &BridgeSharedConfig,
    transport: &Arc<dyn AgentTransport>,
    id: Value,
    value: Value,
) -> Result<(), tokio_tungstenite::tungstenite::Error> {
    use crate::filesystem::handle_write_text_file;

    let params = value.get("params").cloned().unwrap_or_else(|| json!({}));

    // Extract parameters
    let session_id = match params.get("sessionId").and_then(|v| v.as_str()) {
        Some(session_id) => session_id,
        None => {
            send_error_shared(
                &stream,
                id,
                acp::Error::invalid_params()
                    .with_data("missing or invalid sessionId parameter"),
            )
            .await?;
            return Ok(());
        }
    };

    let path = match params.get("path").and_then(|v| v.as_str()) {
        Some(path) => path,
        None => {
            send_error_shared(
                &stream,
                id,
                acp::Error::invalid_params().with_data("missing or invalid path parameter"),
            )
            .await?;
            return Ok(());
        }
    };

    let content = match params.get("content").and_then(|v| v.as_str()) {
        Some(content) => content,
        None => {
            send_error_shared(
                &stream,
                id,
                acp::Error::invalid_params()
                    .with_data("missing or invalid content parameter"),
            )
            .await?;
            return Ok(());
        }
    };

    match handle_write_text_file(
        stream.clone(),
        shared,
        transport,
        session_id,
        path,
        content,
    )
    .await
    {
        Ok(_) => {
            let result = json!({});
            send_result_shared(&stream, id, result).await?;
        }
        Err(error) => {
            send_error_shared(&stream, id, error).await?;
        }
    }
    Ok(())
}

/// Handles the auth/cli_login method
async fn handle_auth_cli_login(
    stream: Arc<TokioMutex<WebSocketStream<TcpStream>>>,
    id: Value,
) -> Result<(), tokio_tungstenite::tungstenite::Error> {
    use crate::auth::handle_auth_cli_login;

    match handle_auth_cli_login().await {
        Ok(login_url) => {
            let result = json!({
                "status": "started",
                "loginUrl": login_url,
            });
            send_result_shared(&stream, id, result).await?;
        }
        Err(error) => {
            send_error_shared(&stream, id, error).await?;
        }
    }
    Ok(())
}

/// Ensures that the bridge metadata is included in the initialize response
///
/// This function implements RAT-LWS-REQ-300 by injecting the bridge ID
/// into the response metadata.
fn ensure_bridge_meta(response: &mut acp::InitializeResponse, bridge_id: &str) {
    use serde_json::Map;
    let mut meta = match response.meta.take() {
        Some(Value::Object(map)) => map,
        _ => Map::new(),
    };
    meta.insert("bridgeId".to_string(), json!(bridge_id));
    response.meta = Some(Value::Object(meta));
}
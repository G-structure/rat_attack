use std::collections::HashMap;
use std::future::Future;
use std::io::{Read, Write};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex, OnceLock,
};
use std::time::Duration;

use agent_client_protocol as acp;
use futures_util::{SinkExt, StreamExt};
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use serde_json::{json, Map, Value};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, oneshot, Mutex as TokioMutex};
use tokio::task::JoinHandle;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::handshake::server::{
    ErrorResponse, Request, Response as HandshakeResponse,
};
use tokio_tungstenite::tungstenite::http::header::{HeaderValue, ORIGIN, SEC_WEBSOCKET_PROTOCOL};
use tokio_tungstenite::tungstenite::http::{Response as HttpResponse, StatusCode};
use tokio_tungstenite::tungstenite::protocol::Message;
use tokio_tungstenite::{accept_hdr_async, tungstenite, WebSocketStream};

#[derive(Clone, Debug)]
pub struct BridgeConfig {
    pub bind_addr: SocketAddr,
    pub allowed_origins: Vec<String>,
    pub expected_subprotocol: String,
    pub bridge_id: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PermissionDecision {
    AllowAlways,
    RejectAlways,
}

pub type PermissionCache = Arc<TokioMutex<HashMap<String, PermissionDecision>>>;

#[derive(Debug)]
pub enum BridgeError {
    Io(std::io::Error),
    Task(tokio::task::JoinError),
}

impl From<std::io::Error> for BridgeError {
    fn from(value: std::io::Error) -> Self {
        BridgeError::Io(value)
    }
}

impl From<tokio::task::JoinError> for BridgeError {
    fn from(value: tokio::task::JoinError) -> Self {
        BridgeError::Task(value)
    }
}

#[derive(Debug)]
pub enum AgentTransportError {
    Protocol(acp::Error),
    Internal(String),
    NotImplemented,
}

impl From<acp::Error> for AgentTransportError {
    fn from(value: acp::Error) -> Self {
        AgentTransportError::Protocol(value)
    }
}

impl AgentTransportError {
    fn into_rpc_error(self) -> acp::Error {
        match self {
            AgentTransportError::Protocol(err) => err,
            AgentTransportError::Internal(message) => {
                acp::Error::internal_error().with_data(message)
            }
            AgentTransportError::NotImplemented => {
                acp::Error::internal_error().with_data("agent transport not implemented")
            }
        }
    }
}

pub struct BridgeHandle {
    local_addr: SocketAddr,
    shutdown: Option<oneshot::Sender<()>>,
    join_handle: Option<JoinHandle<()>>,
}

impl BridgeHandle {
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    pub fn shutdown(
        mut self,
    ) -> Pin<Box<dyn Future<Output = Result<(), BridgeError>> + Send + 'static>> {
        let shutdown = self.shutdown.take();
        let join_handle = self.join_handle.take();

        Box::pin(async move {
            if let Some(sender) = shutdown {
                let _ = sender.send(());
            }

            if let Some(handle) = join_handle {
                handle.await?;
            }

            Ok(())
        })
    }
}

pub trait NotificationSender: Send + Sync {
    fn send_notification(
        &self,
        method: &str,
        params: Value,
    ) -> Pin<Box<dyn Future<Output = Result<(), AgentTransportError>> + Send>>;
}

struct WebSocketNotificationSender {
    stream: Arc<TokioMutex<WebSocketStream<TcpStream>>>,
}

impl WebSocketNotificationSender {
    fn new(stream: Arc<TokioMutex<WebSocketStream<TcpStream>>>) -> Self {
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

pub trait AgentTransport: Send + Sync + 'static {
    fn initialize(
        &self,
        request: acp::InitializeRequest,
    ) -> Pin<Box<dyn Future<Output = Result<acp::InitializeResponse, AgentTransportError>> + Send>>;
    fn new_session(
        &self,
        request: acp::NewSessionRequest,
    ) -> Pin<Box<dyn Future<Output = Result<acp::NewSessionResponse, AgentTransportError>> + Send>>;
    fn prompt(
        &self,
        request: acp::PromptRequest,
        notification_sender: Arc<dyn NotificationSender>,
    ) -> Pin<Box<dyn Future<Output = Result<acp::PromptResponse, AgentTransportError>> + Send>>;
    fn request_permission(
        &self,
        request: acp::RequestPermissionRequest,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<acp::RequestPermissionResponse, AgentTransportError>> + Send,
        >,
    >;
}

pub fn serve(
    config: BridgeConfig,
    transport: Arc<dyn AgentTransport>,
) -> Pin<Box<dyn Future<Output = Result<BridgeHandle, BridgeError>> + Send>> {
    Box::pin(async move {
        let BridgeConfig {
            bind_addr,
            allowed_origins,
            expected_subprotocol,
            bridge_id,
        } = config;

        let listener = TcpListener::bind(bind_addr).await?;
        let local_addr = listener.local_addr()?;

        let shared = Arc::new(BridgeSharedConfig {
            allowed_origins,
            expected_subprotocol,
            bridge_id,
            permission_cache: Arc::new(TokioMutex::new(HashMap::new())),
        });

        let (shutdown_tx, shutdown_rx) = oneshot::channel();

        let join_handle =
            spawn_accept_loop(listener, shutdown_rx, shared.clone(), transport.clone());

        Ok(BridgeHandle {
            local_addr,
            shutdown: Some(shutdown_tx),
            join_handle: Some(join_handle),
        })
    })
}

struct BridgeSharedConfig {
    allowed_origins: Vec<String>,
    expected_subprotocol: String,
    bridge_id: String,
    permission_cache: PermissionCache,
}

fn spawn_accept_loop(
    listener: TcpListener,
    mut shutdown_rx: oneshot::Receiver<()>,
    shared: Arc<BridgeSharedConfig>,
    transport: Arc<dyn AgentTransport>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = &mut shutdown_rx => {
                    break;
                }
                accept_result = listener.accept() => {
                    let (stream, _) = match accept_result {
                        Ok(pair) => pair,
                        Err(_) => break,
                    };
                    let shared = shared.clone();
                    let transport = transport.clone();
                    tokio::spawn(async move {
                        if let Err(err) = handle_client(stream, shared, transport).await {
                            match err {
                                ClientError::Handshake(error) | ClientError::WebSocket(error) => {
                                    drop(error); // TODO: replace with structured logging
                                }
                            }
                        }
                    });
                }
            }
        }
    })
}

enum ClientError {
    Handshake(tungstenite::Error),
    WebSocket(tungstenite::Error),
}

async fn handle_client(
    stream: TcpStream,
    shared: Arc<BridgeSharedConfig>,
    transport: Arc<dyn AgentTransport>,
) -> Result<(), ClientError> {
    let ws_stream = accept_client(stream, shared.clone())
        .await
        .map_err(ClientError::Handshake)?;
    handle_websocket(ws_stream, shared, transport)
        .await
        .map_err(ClientError::WebSocket)
}

async fn accept_client(
    stream: TcpStream,
    shared: Arc<BridgeSharedConfig>,
) -> Result<WebSocketStream<TcpStream>, tungstenite::Error> {
    let allowed_origins = shared.allowed_origins.clone();
    let expected_subprotocol = shared.expected_subprotocol.clone();

    accept_hdr_async(
        stream,
        move |request: &Request, mut response: HandshakeResponse| {
            validate_origin(request, &allowed_origins)?;
            validate_subprotocol(request, &mut response, &expected_subprotocol)?;
            Ok(response)
        },
    )
    .await
}

#[allow(clippy::result_large_err)]
fn validate_origin(request: &Request, allowed_origins: &[String]) -> Result<(), ErrorResponse> {
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

#[allow(clippy::result_large_err)]
fn validate_subprotocol(
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

fn handshake_error(status: StatusCode, message: &str) -> ErrorResponse {
    HttpResponse::builder()
        .status(status)
        .body(Some(message.to_owned()))
        .unwrap_or_else(|_| HttpResponse::builder().status(status).body(None).unwrap())
}

async fn handle_websocket(
    stream: WebSocketStream<TcpStream>,
    shared: Arc<BridgeSharedConfig>,
    transport: Arc<dyn AgentTransport>,
) -> Result<(), tungstenite::Error> {
    let stream = Arc::new(TokioMutex::new(stream));
    let mut initialized = false;

    loop {
        let message = {
            let mut stream_guard = stream.lock().await;
            stream_guard.next().await
        };

        match message {
            Some(Ok(Message::Text(text))) => {
                let value: Value = match serde_json::from_str(&text) {
                    Ok(value) => value,
                    Err(_) => {
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
                let value: Value = match serde_json::from_slice(&bytes) {
                    Ok(value) => value,
                    Err(_) => {
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
                let mut stream_guard = stream.lock().await;
                stream_guard.send(Message::Pong(payload)).await?;
            }
            Some(Ok(Message::Pong(_))) => {}
            Some(Ok(Message::Close(_))) | None => {
                break;
            }
            Some(Ok(Message::Frame(_))) => {}
            Some(Err(e)) => return Err(e),
        }
    }

    Ok(())
}

async fn process_request(
    stream: Arc<TokioMutex<WebSocketStream<TcpStream>>>,
    shared: &BridgeSharedConfig,
    transport: &Arc<dyn AgentTransport>,
    initialized: &mut bool,
    value: Value,
) -> Result<(), tungstenite::Error> {
    let id = value.get("id").cloned().unwrap_or(Value::Null);
    let method = value.get("method").and_then(|value| value.as_str());

    let method = match method {
        Some(method) => method,
        None => {
            send_error_shared(&stream, id, acp::Error::invalid_request()).await?;
            return Ok(());
        }
    };

    match method {
        "initialize" => {
            let params = value.get("params").cloned().unwrap_or_else(|| json!({}));
            let request: acp::InitializeRequest = match serde_json::from_value(params) {
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

            let response = transport.initialize(request).await;
            match response {
                Ok(mut response) => {
                    ensure_bridge_meta(&mut response, &shared.bridge_id);
                    let result = serde_json::to_value(response)
                        .map_err(|err| tungstenite::Error::Io(std::io::Error::other(err)))?;
                    send_result_shared(&stream, id, result).await?;
                    *initialized = true;
                }
                Err(err) => {
                    let error = err.into_rpc_error();
                    send_error_shared(&stream, id, error).await?;
                }
            }
        }
        "session/new" => {
            if !*initialized {
                let error = acp::Error::method_not_found();
                send_error_shared(&stream, id, error).await?;
                return Ok(());
            }

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
                        .map_err(|err| tungstenite::Error::Io(std::io::Error::other(err)))?;
                    send_result_shared(&stream, id, result).await?;
                }
                Err(err) => {
                    let error = err.into_rpc_error();
                    send_error_shared(&stream, id, error).await?;
                }
            }
        }
        "session/prompt" => {
            if !*initialized {
                let error = acp::Error::method_not_found();
                send_error_shared(&stream, id, error).await?;
                return Ok(());
            }

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
                        .map_err(|err| tungstenite::Error::Io(std::io::Error::other(err)))?;
                    send_result_shared(&stream, id, result).await?;
                }
                Err(err) => {
                    let error = err.into_rpc_error();
                    send_error_shared(&stream, id, error).await?;
                }
            }
        }
        "fs/read_text_file" => {
            if !*initialized {
                let error = acp::Error::method_not_found();
                send_error_shared(&stream, id, error).await?;
                return Ok(());
            }

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
        }
        "fs/write_text_file" => {
            if !*initialized {
                let error = acp::Error::method_not_found();
                send_error_shared(&stream, id, error).await?;
                return Ok(());
            }

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
        }
        "auth/cli_login" => match handle_auth_cli_login().await {
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
        },
        _ => {
            let error = acp::Error::method_not_found();
            send_error_shared(&stream, id, error).await?;
        }
    }

    Ok(())
}

// TODO: Improve project root determination and overhaul sandboxing logic.
// The current implementation blocks a set of hardcoded system directories
// and resolves relative paths against the current working directory.
// This approach is fragile and may allow directory traversal or
// unintended access to files outside the intended project root.
// Future work should compute the actual project root (e.g., via
// environment variables, a .git directory, or a configuration file)
// and enforce that all file accesses stay within that root.
fn validate_and_resolve_path(path: &str, for_write: bool) -> Result<PathBuf, acp::Error> {
    let path_buf = PathBuf::from(path);

    // Implement project root sandboxing per RAT-LWS-REQ-044
    // Block access to sensitive system paths
    if path.starts_with("/etc/")
        || path.starts_with("/var/")
        || path.starts_with("/root/")
        || path.starts_with("/usr/")
        || path.starts_with("/boot/")
        || path.starts_with("/proc/")
    {
        return Err(acp::Error::internal_error().with_data("path outside project root"));
    }

    // For relative paths, resolve against current working directory
    let resolved_path = if path_buf.is_absolute() {
        path_buf
    } else {
        std::env::current_dir()
            .map_err(|_| acp::Error::internal_error().with_data("failed to get current directory"))?
            .join(&path_buf)
    };

    // Canonicalize path, handling the case where file doesn't exist for writes
    let canonical_path = if for_write && !resolved_path.exists() {
        // For write operations, canonicalize the parent directory since the file may not exist yet
        let parent = resolved_path
            .parent()
            .ok_or_else(|| acp::Error::internal_error().with_data("invalid path"))?;
        let canonical_parent = parent
            .canonicalize()
            .map_err(|_| acp::Error::internal_error().with_data("invalid path"))?;
        canonical_parent.join(
            resolved_path
                .file_name()
                .ok_or_else(|| acp::Error::internal_error().with_data("invalid path"))?,
        )
    } else {
        resolved_path.canonicalize().map_err(|_| {
            acp::Error::internal_error().with_data(if for_write {
                "invalid path"
            } else {
                "file not found"
            })
        })?
    };

    // Additional safety check: ensure the canonical path doesn't escape to system directories
    let canonical_str = canonical_path.to_string_lossy();
    if canonical_str.starts_with("/etc/")
        || canonical_str.starts_with("/var/")
        || canonical_str.starts_with("/root/")
        || canonical_str.starts_with("/usr/")
        || canonical_str.starts_with("/boot/")
        || canonical_str.starts_with("/proc/")
    {
        return Err(acp::Error::internal_error().with_data("path outside project root"));
    }

    Ok(canonical_path)
}

fn handle_read_text_file(
    path: &str,
    line_offset: Option<u32>,
    line_limit: Option<u32>,
) -> Result<String, acp::Error> {
    let canonical_path = validate_and_resolve_path(path, false)?;

    // First read as bytes to check for binary content
    let bytes = std::fs::read(&canonical_path)
        .map_err(|_| acp::Error::internal_error().with_data("file not found"))?;

    // Check if it's likely a binary file (contains null bytes)
    if bytes.contains(&0) {
        return Err(acp::Error::internal_error().with_data("binary file not supported"));
    }

    // Convert to string
    let content = String::from_utf8(bytes)
        .map_err(|_| acp::Error::internal_error().with_data("file contains invalid UTF-8"))?;

    apply_line_filter(&content, line_offset, line_limit)
}

fn apply_line_filter(
    content: &str,
    line_offset: Option<u32>,
    line_limit: Option<u32>,
) -> Result<String, acp::Error> {
    let lines: Vec<&str> = content.lines().collect();

    match (line_offset, line_limit) {
        (Some(offset), Some(limit)) => {
            let start_idx = (offset.saturating_sub(1) as usize).min(lines.len());
            let end_idx = (start_idx + limit as usize).min(lines.len());

            if start_idx >= lines.len() {
                Ok(String::new())
            } else {
                Ok(lines[start_idx..end_idx].join("\n"))
            }
        }
        (Some(offset), None) => {
            let start_idx = (offset.saturating_sub(1) as usize).min(lines.len());

            if start_idx >= lines.len() {
                Ok(String::new())
            } else {
                Ok(lines[start_idx..].join("\n"))
            }
        }
        (None, Some(limit)) => {
            let end_idx = (limit as usize).min(lines.len());
            Ok(lines[..end_idx].join("\n"))
        }
        (None, None) => Ok(content.to_string()),
    }
}

// TODO: Refactor permission handling into a generic monadic abstraction so it can be more generally applied to different tools.
async fn handle_write_text_file(
    _stream: Arc<TokioMutex<WebSocketStream<TcpStream>>>,
    shared: &BridgeSharedConfig,
    transport: &Arc<dyn AgentTransport>,
    session_id: &str,
    path: &str,
    content: &str,
) -> Result<(), acp::Error> {
    use std::fs;

    // First, check sandboxing
    let canonical_path = validate_and_resolve_path(path, true)?;
    let canonical_path_str = canonical_path.to_string_lossy().to_string();

    // Create parent directories if they don't exist
    if let Some(parent) = canonical_path.parent() {
        fs::create_dir_all(parent).map_err(|_| {
            acp::Error::internal_error().with_data("failed to create parent directories")
        })?;
    }

    // Check permission cache first
    let cached_decision = {
        let cache = shared.permission_cache.lock().await;
        cache.get(&canonical_path_str).cloned()
    };

    match cached_decision {
        Some(PermissionDecision::AllowAlways) => {
            // Cached allow_always - proceed with write without requesting permission
            fs::write(&canonical_path, content)
                .map_err(|_| acp::Error::internal_error().with_data("failed to write file"))?;
            return Ok(());
        }
        Some(PermissionDecision::RejectAlways) => {
            // Cached reject_always - return error immediately
            return Err(acp::Error::new((-32000, "Permission denied".to_string())));
        }
        None => {
            // No cached decision - request permission from agent
        }
    }

    // Request permission from the agent
    let permission_request = acp::RequestPermissionRequest {
        session_id: acp::SessionId(session_id.to_string().into()),
        tool_call: acp::ToolCallUpdate {
            id: acp::ToolCallId("fs_write_text_file".to_string().into()),
            fields: acp::ToolCallUpdateFields {
                kind: Some(acp::ToolKind::Edit),
                title: Some(format!("Write file: {path}")),
                status: Some(acp::ToolCallStatus::InProgress),
                ..Default::default()
            },
            meta: None,
        },
        options: vec![
            acp::PermissionOption {
                id: acp::PermissionOptionId("allow_once".to_string().into()),
                name: "Allow this write operation".to_string(),
                kind: acp::PermissionOptionKind::AllowOnce,
                meta: None,
            },
            acp::PermissionOption {
                id: acp::PermissionOptionId("allow_always".to_string().into()),
                name: "Allow all write operations".to_string(),
                kind: acp::PermissionOptionKind::AllowAlways,
                meta: None,
            },
            acp::PermissionOption {
                id: acp::PermissionOptionId("reject_once".to_string().into()),
                name: "Reject this write operation".to_string(),
                kind: acp::PermissionOptionKind::RejectOnce,
                meta: None,
            },
            acp::PermissionOption {
                id: acp::PermissionOptionId("reject_always".to_string().into()),
                name: "Reject all write operations".to_string(),
                kind: acp::PermissionOptionKind::RejectAlways,
                meta: None,
            },
        ],
        meta: None,
    };

    let permission_response = transport
        .request_permission(permission_request)
        .await
        .map_err(|_| acp::Error::internal_error().with_data("permission request failed"))?;

    // Check the permission outcome and update cache
    match permission_response.outcome {
        acp::RequestPermissionOutcome::Selected { option_id } => {
            match option_id.0.as_ref() {
                "allow_once" => {
                    // Permission granted for this write only, proceed with write
                    fs::write(&canonical_path, content).map_err(|_| {
                        acp::Error::internal_error().with_data("failed to write file")
                    })?;
                    Ok(())
                }
                "allow_always" => {
                    // Permission granted always, cache the decision and proceed with write
                    {
                        let mut cache = shared.permission_cache.lock().await;
                        cache.insert(canonical_path_str, PermissionDecision::AllowAlways);
                    }
                    fs::write(&canonical_path, content).map_err(|_| {
                        acp::Error::internal_error().with_data("failed to write file")
                    })?;
                    Ok(())
                }
                "reject_once" => {
                    // Permission denied for this write only
                    Err(acp::Error::new((-32000, "Permission denied".to_string())))
                }
                "reject_always" => {
                    // Permission denied always, cache the decision
                    {
                        let mut cache = shared.permission_cache.lock().await;
                        cache.insert(canonical_path_str, PermissionDecision::RejectAlways);
                    }
                    Err(acp::Error::new((-32000, "Permission denied".to_string())))
                }
                _ => {
                    // Unknown option
                    Err(acp::Error::new((
                        -32000,
                        "Unknown permission option".to_string(),
                    )))
                }
            }
        }
        acp::RequestPermissionOutcome::Cancelled => {
            // Permission request was cancelled
            Err(acp::Error::new((
                -32000,
                "Permission request cancelled".to_string(),
            )))
        }
    }
}

async fn handle_auth_cli_login() -> Result<String, acp::Error> {
    let (cli_path, args) = resolve_claude_login_command()?;

    let project_root = std::env::current_dir()
        .map_err(|_| acp::Error::internal_error().with_data("failed to get current directory"))?;

    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|err| {
            acp::Error::internal_error().with_data(format!("failed to open pty: {err}"))
        })?;

    let cli_command = cli_path
        .to_str()
        .ok_or_else(|| acp::Error::internal_error().with_data("invalid Claude CLI path"))?
        .to_string();

    let mut builder = CommandBuilder::new(cli_command);
    for arg in &args {
        builder.arg(arg);
    }
    builder.arg("/login");
    builder.cwd(&project_root);
    for (key, value) in std::env::vars() {
        builder.env(key, value);
    }

    let child = pair.slave.spawn_command(builder).map_err(|err| {
        acp::Error::internal_error().with_data(format!("failed to spawn login CLI: {err}"))
    })?;
    drop(pair.slave);

    let mut reader = pair.master.try_clone_reader().map_err(|err| {
        acp::Error::internal_error().with_data(format!("failed to clone pty reader: {err}"))
    })?;
    let mut writer = pair.master.take_writer().map_err(|err| {
        acp::Error::internal_error().with_data(format!("failed to take pty writer: {err}"))
    })?;

    let automation_stop = Arc::new(AtomicBool::new(false));
    let writer_stop = automation_stop.clone();
    let writer_thread = std::thread::spawn(move || {
        while !writer_stop.load(Ordering::Relaxed) {
            if writer.write_all(b"\r").is_err() {
                break;
            }
            let _ = writer.flush();
            std::thread::sleep(Duration::from_millis(250));
        }
    });

    let (tx, mut rx) = mpsc::unbounded_channel();
    let reader_stop = automation_stop.clone();
    let reader_thread = std::thread::spawn(move || {
        let mut buffer = [0u8; 4096];
        while !reader_stop.load(Ordering::Relaxed) {
            match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(read) => {
                    if tx.send(buffer[..read].to_vec()).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    let capture_stop = automation_stop.clone();
    let capture = async move {
        let mut collected = String::new();
        while let Some(chunk) = rx.recv().await {
            let text = String::from_utf8_lossy(&chunk);
            collected.push_str(&text);
            if let Some(url) = extract_login_url(&collected) {
                capture_stop.store(true, Ordering::Relaxed);
                return Ok::<String, acp::Error>(url);
            }
        }

        capture_stop.store(true, Ordering::Relaxed);
        Err(acp::Error::internal_error().with_data("login CLI exited before emitting a login URL"))
    };

    let capture_result = timeout(Duration::from_secs(30), capture).await;

    automation_stop.store(true, Ordering::Relaxed);
    let _ = writer_thread.join();
    let _ = reader_thread.join();

    let result = capture_result.map_err(|_| {
        acp::Error::internal_error().with_data("timed out waiting for Claude login URL")
    })?;

    // Detach the child process; the CLI continues running until the user completes login.
    drop(child);

    result
}

// Global mutex to serialize CLI resolution during tests to prevent env var races
static CLI_RESOLUTION_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn resolve_claude_login_command() -> Result<(PathBuf, Vec<String>), acp::Error> {
    // Serialize access to environment variables during resolution to prevent test interference
    let lock = CLI_RESOLUTION_LOCK.get_or_init(|| Mutex::new(()));
    let _guard = lock.lock().unwrap();

    // Check for test failure mode
    if std::env::var("TEST_MODE_FAIL").is_ok() {
        return Err(acp::Error::new((-32000, "Unable to locate Claude login CLI. Try installing @zed-industries/claude-code-acp or ensure `claude` is in PATH.".to_string())));
    }

    // Check for test-specific override first (highest priority for tests)
    if let Ok(path) = std::env::var("TEST_CLAUDE_CLI_PATH") {
        return Ok((PathBuf::from(path), vec![]));
    }

    // Check CLAUDE_ACP_BIN environment variable
    if let Ok(path) = std::env::var("CLAUDE_ACP_BIN") {
        let path_buf = PathBuf::from(path);
        if path_buf.exists() {
            return Ok((path_buf, vec![]));
        }
    }

    // Try to find Claude Code CLI from node_modules (like Zed does)
    if let Some((path, args)) = find_claude_code_cli_from_node_modules() {
        return Ok((path, args));
    }

    // Fallback: try a `claude` executable in PATH
    if let Ok(path) = which::which("claude") {
        return Ok((path, vec![]));
    }

    Err(acp::Error::new((-32000, "Unable to locate Claude login CLI. Try installing @zed-industries/claude-code-acp or ensure `claude` is in PATH.".to_string())))
}

fn find_claude_code_cli_from_node_modules() -> Option<(PathBuf, Vec<String>)> {
    // Look for the Claude Code CLI in node_modules, similar to Zed's approach
    // Check if we have @zed-industries/claude-code-acp installed locally
    let acp_entry = PathBuf::from("node_modules/@zed-industries/claude-code-acp/dist/index.js");
    if acp_entry.exists() {
        // Walk up to find the @anthropic-ai/claude-code/cli.js
        let node_modules_dir = acp_entry
            .parent() // dist
            .and_then(|p| p.parent()) // @zed-industries/claude-code-acp
            .and_then(|p| p.parent()) // @zed-industries
            .and_then(|p| p.parent()); // node_modules

        if let Some(node_modules_dir) = node_modules_dir {
            let cli_js = node_modules_dir
                .join("@anthropic-ai")
                .join("claude-code")
                .join("cli.js");
            if cli_js.exists() {
                return Some((
                    PathBuf::from("node"),
                    vec![cli_js.to_string_lossy().to_string()],
                ));
            }
        }
    }

    None
}

fn extract_login_url(buffer: &str) -> Option<String> {
    let start = buffer.find("https://")?;
    let tail = &buffer[start..];
    let mut end = tail.len();
    for (idx, ch) in tail.char_indices() {
        if ch.is_whitespace() || ch == '"' || ch == '\'' || ch == '\u{7}' || ch == '\u{1b}' {
            end = idx;
            break;
        }
    }
    let mut url = tail[..end].to_string();
    if let Some(pos) = url.find('\u{7}') {
        url.truncate(pos);
    }
    if let Some(pos) = url.find('\u{1b}') {
        url.truncate(pos);
    }
    if url.is_empty() {
        None
    } else {
        Some(url)
    }
}

fn ensure_bridge_meta(response: &mut acp::InitializeResponse, bridge_id: &str) {
    let mut meta = match response.meta.take() {
        Some(Value::Object(map)) => map,
        _ => Map::new(),
    };
    meta.insert("bridgeId".to_string(), json!(bridge_id));
    response.meta = Some(Value::Object(meta));
}

async fn send_result(
    stream: &mut WebSocketStream<TcpStream>,
    id: Value,
    result: Value,
) -> Result<(), tungstenite::Error> {
    let payload = json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result,
    });
    send_json(stream, payload).await
}

async fn send_error(
    stream: &mut WebSocketStream<TcpStream>,
    id: Value,
    error: acp::Error,
) -> Result<(), tungstenite::Error> {
    let payload = json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": error,
    });
    send_json(stream, payload).await
}

async fn send_result_shared(
    stream: &Arc<TokioMutex<WebSocketStream<TcpStream>>>,
    id: Value,
    result: Value,
) -> Result<(), tungstenite::Error> {
    let mut guard = stream.lock().await;
    send_result(&mut guard, id, result).await
}

async fn send_error_shared(
    stream: &Arc<TokioMutex<WebSocketStream<TcpStream>>>,
    id: Value,
    error: acp::Error,
) -> Result<(), tungstenite::Error> {
    let mut guard = stream.lock().await;
    send_error(&mut guard, id, error).await
}

async fn send_json(
    stream: &mut WebSocketStream<TcpStream>,
    payload: Value,
) -> Result<(), tungstenite::Error> {
    let text = serde_json::to_string(&payload)
        .map_err(|err| tungstenite::Error::Io(std::io::Error::other(err)))?;
    stream.send(Message::Text(text)).await
}

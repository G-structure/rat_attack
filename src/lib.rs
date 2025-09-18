use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;

use agent_client_protocol as acp;
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Map, Value};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{oneshot, Mutex as TokioMutex};
use tokio::task::JoinHandle;
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
            send_json(&mut *guard, payload)
                .await
                .map_err(|_| AgentTransportError::Internal("Failed to send notification".to_string()))?;
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
                        send_error(&mut *stream_guard, Value::Null, acp::Error::parse_error()).await?;
                        continue;
                    }
                };
                process_request(stream.clone(), &shared, &transport, &mut initialized, value).await?;
            }
            Some(Ok(Message::Binary(bytes))) => {
                let value: Value = match serde_json::from_slice(&bytes) {
                    Ok(value) => value,
                    Err(_) => {
                        let mut stream_guard = stream.lock().await;
                        send_error(&mut *stream_guard, Value::Null, acp::Error::parse_error()).await?;
                        continue;
                    }
                };
                process_request(stream.clone(), &shared, &transport, &mut initialized, value).await?;
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
            let session_id = params.get("sessionId")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let prompt_text = params.get("prompt")
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
        _ => {
            let error = acp::Error::method_not_found();
            send_error_shared(&stream, id, error).await?;
        }
    }

    Ok(())
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
    send_result(&mut *guard, id, result).await
}

async fn send_error_shared(
    stream: &Arc<TokioMutex<WebSocketStream<TcpStream>>>,
    id: Value,
    error: acp::Error,
) -> Result<(), tungstenite::Error> {
    let mut guard = stream.lock().await;
    send_error(&mut *guard, id, error).await
}

async fn send_json(
    stream: &mut WebSocketStream<TcpStream>,
    payload: Value,
) -> Result<(), tungstenite::Error> {
    let text = serde_json::to_string(&payload)
        .map_err(|err| tungstenite::Error::Io(std::io::Error::other(err)))?;
    stream.send(Message::Text(text)).await
}

//! Stdio agent transport for external ACP agents
//!
//! This module provides a transport that spawns external agent processes
//! and communicates with them over stdio using JSON-RPC.

use std::{
    future::Future,
    path::PathBuf,
    pin::Pin,
    process::Stdio,
    sync::Arc,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::{Child, Command},
    sync::{mpsc, Mutex},
    task::JoinHandle,
};
use agent_client_protocol as acp;
use serde_json::{json, Value};
use futures_util::FutureExt;

use crate::transport::{AgentTransport, AgentTransportError, NotificationSender};

/// A transport that communicates with external agents over stdio
pub struct StdioAgentTransport {
    command: String,
    args: Vec<String>,
    project_root: PathBuf,
    process: Arc<Mutex<Option<Child>>>,
    request_sender: Arc<Mutex<Option<mpsc::UnboundedSender<StdioRequest>>>>,
    notification_sender: Arc<Mutex<Option<Arc<dyn NotificationSender>>>>,
    _task_handle: JoinHandle<()>,
}

#[derive(Debug)]
struct StdioRequest {
    method: String,
    params: Value,
    response_sender: tokio::sync::oneshot::Sender<Result<Value, AgentTransportError>>,
}

impl StdioAgentTransport {
    /// Creates a new stdio agent transport
    pub async fn new(
        command: &str,
        args: Vec<String>,
        project_root: PathBuf,
    ) -> Result<Self, AgentTransportError> {
        let mut child = Command::new(command)
            .args(&args)
            .current_dir(&project_root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| AgentTransportError::ProcessSpawn(format!("Failed to spawn {}: {}", command, e)))?;

        let stdin = child.stdin.take().ok_or_else(|| {
            AgentTransportError::ProcessSpawn("Failed to get stdin".to_string())
        })?;

        let stdout = child.stdout.take().ok_or_else(|| {
            AgentTransportError::ProcessSpawn("Failed to get stdout".to_string())
        })?;

        let (request_tx, mut request_rx) = mpsc::unbounded_channel::<StdioRequest>();
        let notification_sender = Arc::new(Mutex::new(None::<Arc<dyn NotificationSender>>));
        let notification_sender_clone = notification_sender.clone();

        // Spawn task to handle stdio communication
        let task_handle = tokio::spawn(async move {
            let mut reader = BufReader::new(stdout);
            let mut writer = stdin;
            let mut pending_requests: std::collections::HashMap<String, tokio::sync::oneshot::Sender<Result<Value, AgentTransportError>>> = std::collections::HashMap::new();
            let mut request_id_counter = 0u64;

            loop {
                tokio::select! {
                    // Handle outgoing requests
                    Some(request) = request_rx.recv() => {
                        request_id_counter += 1;
                        let request_id = request_id_counter.to_string();

                        let json_request = json!({
                            "jsonrpc": "2.0",
                            "id": request_id,
                            "method": request.method,
                            "params": request.params
                        });

                        if let Ok(request_line) = serde_json::to_string(&json_request) {
                            let request_line = format!("{}\n", request_line);
                            if writer.write_all(request_line.as_bytes()).await.is_ok() {
                                pending_requests.insert(request_id, request.response_sender);
                            } else {
                                let _ = request.response_sender.send(Err(AgentTransportError::Communication("Failed to write to agent".to_string())));
                            }
                        } else {
                            let _ = request.response_sender.send(Err(AgentTransportError::Communication("Failed to serialize request".to_string())));
                        }
                    }

                    // Handle incoming responses
                    line_result = async {
                        let mut line = String::new();
                        reader.read_line(&mut line).await.map(|n| (n, line))
                    } => {
                        match line_result {
                            Ok((0, _)) => break, // EOF
                            Ok((_, line)) => {
                                if let Ok(response) = serde_json::from_str::<Value>(&line) {
                                    if let Some(id) = response.get("id").and_then(|v| v.as_str()) {
                                        // This is a response to a request
                                        if let Some(sender) = pending_requests.remove(id) {
                                            if let Some(result) = response.get("result") {
                                                let _ = sender.send(Ok(result.clone()));
                                            } else if let Some(error) = response.get("error") {
                                                let error_msg = error.get("message")
                                                    .and_then(|m| m.as_str())
                                                    .unwrap_or("Unknown error");
                                                let _ = sender.send(Err(AgentTransportError::Communication(error_msg.to_string())));
                                            }
                                        }
                                    } else if let Some(method) = response.get("method").and_then(|m| m.as_str()) {
                                        // This is a notification - forward it to the WebSocket client
                                        if let Some(params) = response.get("params") {
                                            let sender_guard = notification_sender.lock().await;
                                            if let Some(ref sender) = *sender_guard {
                                                let _ = sender.send_notification(method, params.clone()).await;
                                            }
                                        }
                                    }
                                }
                            }
                            Err(_) => break,
                        }
                    }
                }
            }
        });

        Ok(Self {
            command: command.to_string(),
            args,
            project_root,
            process: Arc::new(Mutex::new(Some(child))),
            request_sender: Arc::new(Mutex::new(Some(request_tx))),
            notification_sender: notification_sender_clone,
            _task_handle: task_handle,
        })
    }

    async fn send_request(&self, method: &str, params: Value) -> Result<Value, AgentTransportError> {
        let sender = self.request_sender.lock().await;
        let sender = sender.as_ref().ok_or_else(|| {
            AgentTransportError::Communication("Transport not initialized".to_string())
        })?;

        let (response_tx, response_rx) = tokio::sync::oneshot::channel();
        let request = StdioRequest {
            method: method.to_string(),
            params,
            response_sender: response_tx,
        };

        sender.send(request).map_err(|_| {
            AgentTransportError::Communication("Failed to send request".to_string())
        })?;

        response_rx.await.map_err(|_| {
            AgentTransportError::Communication("Failed to receive response".to_string())
        })?
    }
}

impl AgentTransport for StdioAgentTransport {
    fn initialize(
        &self,
        request: acp::InitializeRequest,
    ) -> Pin<Box<dyn Future<Output = Result<acp::InitializeResponse, AgentTransportError>> + Send>> {
        let params = serde_json::to_value(request).unwrap_or(json!({}));
        let transport = self.request_sender.clone();

        async move {
            let sender = transport.lock().await;
            let sender = sender.as_ref().ok_or_else(|| {
                AgentTransportError::Communication("Transport not initialized".to_string())
            })?;

            let (response_tx, response_rx) = tokio::sync::oneshot::channel();
            let request = StdioRequest {
                method: "initialize".to_string(),
                params,
                response_sender: response_tx,
            };

            sender.send(request).map_err(|_| {
                AgentTransportError::Communication("Failed to send request".to_string())
            })?;

            let result = response_rx.await.map_err(|_| {
                AgentTransportError::Communication("Failed to receive response".to_string())
            })??;

            serde_json::from_value(result).map_err(|e| {
                AgentTransportError::Communication(format!("Failed to parse initialize response: {}", e))
            })
        }.boxed()
    }

    fn new_session(
        &self,
        request: acp::NewSessionRequest,
    ) -> Pin<Box<dyn Future<Output = Result<acp::NewSessionResponse, AgentTransportError>> + Send>> {
        let params = serde_json::to_value(request).unwrap_or(json!({}));
        let transport = self.request_sender.clone();

        async move {
            let sender = transport.lock().await;
            let sender = sender.as_ref().ok_or_else(|| {
                AgentTransportError::Communication("Transport not initialized".to_string())
            })?;

            let (response_tx, response_rx) = tokio::sync::oneshot::channel();
            let request = StdioRequest {
                method: "session/new".to_string(),
                params,
                response_sender: response_tx,
            };

            sender.send(request).map_err(|_| {
                AgentTransportError::Communication("Failed to send request".to_string())
            })?;

            let result = response_rx.await.map_err(|_| {
                AgentTransportError::Communication("Failed to receive response".to_string())
            })??;

            serde_json::from_value(result).map_err(|e| {
                AgentTransportError::Communication(format!("Failed to parse session response: {}", e))
            })
        }.boxed()
    }

    fn prompt(
        &self,
        request: acp::PromptRequest,
        notification_sender: Arc<dyn NotificationSender>,
    ) -> Pin<Box<dyn Future<Output = Result<acp::PromptResponse, AgentTransportError>> + Send>> {
        let params = serde_json::to_value(request).unwrap_or(json!({}));
        let transport = self.request_sender.clone();
        let notification_store = self.notification_sender.clone();

        async move {
            // Store the notification sender for the stdio loop to use
            {
                let mut ns = notification_store.lock().await;
                *ns = Some(notification_sender);
            }

            let sender = transport.lock().await;
            let sender = sender.as_ref().ok_or_else(|| {
                AgentTransportError::Communication("Transport not initialized".to_string())
            })?;

            let (response_tx, response_rx) = tokio::sync::oneshot::channel();
            let request = StdioRequest {
                method: "session/prompt".to_string(),
                params,
                response_sender: response_tx,
            };

            sender.send(request).map_err(|_| {
                AgentTransportError::Communication("Failed to send request".to_string())
            })?;

            let result = response_rx.await.map_err(|_| {
                AgentTransportError::Communication("Failed to receive response".to_string())
            })??;

            serde_json::from_value(result).map_err(|e| {
                AgentTransportError::Communication(format!("Failed to parse prompt response: {}", e))
            })
        }.boxed()
    }

    fn request_permission(
        &self,
        request: acp::RequestPermissionRequest,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<acp::RequestPermissionResponse, AgentTransportError>> + Send,
        >,
    > {
        let params = serde_json::to_value(request).unwrap_or(json!({}));
        let transport = self.request_sender.clone();

        async move {
            let sender = transport.lock().await;
            let sender = sender.as_ref().ok_or_else(|| {
                AgentTransportError::Communication("Transport not initialized".to_string())
            })?;

            let (response_tx, response_rx) = tokio::sync::oneshot::channel();
            let request = StdioRequest {
                method: "session/request_permission".to_string(),
                params,
                response_sender: response_tx,
            };

            sender.send(request).map_err(|_| {
                AgentTransportError::Communication("Failed to send request".to_string())
            })?;

            let result = response_rx.await.map_err(|_| {
                AgentTransportError::Communication("Failed to receive response".to_string())
            })??;

            serde_json::from_value(result).map_err(|e| {
                AgentTransportError::Communication(format!("Failed to parse permission response: {}", e))
            })
        }.boxed()
    }
}
//! Stdio agent transport for external ACP agents
//!
//! This module provides a transport that spawns external agent processes
//! and communicates with them over stdio using JSON-RPC.

use agent_client_protocol as acp;
use futures_util::FutureExt;
use serde_json::{json, Value};
use std::{future::Future, path::PathBuf, pin::Pin, process::Stdio, sync::Arc};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::{Child, Command},
    sync::{mpsc, Mutex},
    task::JoinHandle,
};

use crate::transport::{AgentTransport, AgentTransportError, NotificationSender};

/// A transport that communicates with external agents over stdio
pub struct StdioAgentTransport {
    #[allow(dead_code)]
    command: String,
    #[allow(dead_code)]
    args: Vec<String>,
    #[allow(dead_code)]
    project_root: PathBuf,
    #[allow(dead_code)]
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
            .map_err(|e| {
                AgentTransportError::ProcessSpawn(format!("Failed to spawn {command}: {e}"))
            })?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| AgentTransportError::ProcessSpawn("Failed to get stdin".to_string()))?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| AgentTransportError::ProcessSpawn("Failed to get stdout".to_string()))?;

        let (request_tx, mut request_rx) = mpsc::unbounded_channel::<StdioRequest>();
        let notification_sender = Arc::new(Mutex::new(None::<Arc<dyn NotificationSender>>));
        let notification_sender_clone = notification_sender.clone();
        let project_root_clone = project_root.clone();

        // Spawn task to handle stdio communication
        let task_handle = tokio::spawn(async move {
            let mut reader = BufReader::new(stdout);
            let mut writer = stdin;
            let mut pending_requests: std::collections::HashMap<
                String,
                tokio::sync::oneshot::Sender<Result<Value, AgentTransportError>>,
            > = std::collections::HashMap::new();
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
                            let request_line = format!("{request_line}\n");
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
                                        // Check if this is a file system method that should be handled by the bridge
                                        match method {
                                            "fs/write_text_file" => {
                                                tracing::debug!("Handling fs/write_text_file request from agent");

                                                // Parse the request parameters
                                                if let Some(params) = response.get("params") {
                                                    let path = params.get("path").and_then(|p| p.as_str()).unwrap_or("");
                                                    let content = params.get("content").and_then(|c| c.as_str()).unwrap_or("");

                                                    // Simple file write relative to project root
                                                    let file_path = if std::path::Path::new(path).is_absolute() {
                                                        PathBuf::from(path)
                                                    } else {
                                                        project_root_clone.join(path)
                                                    };

                                                    // Write the file
                                                    match std::fs::write(&file_path, content) {
                                                        Ok(_) => {
                                                            tracing::info!(path = %file_path.display(), "Successfully wrote file");

                                                            // Send success response back to agent if it has an ID
                                                            if let Some(id) = response.get("id") {
                                                                let success_response = json!({
                                                                    "jsonrpc": "2.0",
                                                                    "id": id,
                                                                    "result": {}
                                                                });

                                                                let response_line = format!("{}\n", success_response);
                                                                let _ = writer.write_all(response_line.as_bytes()).await;
                                                            }
                                                        }
                                                        Err(e) => {
                                                            tracing::error!(path = %file_path.display(), error = %e, "Failed to write file");

                                                            // Send error response back to agent if it has an ID
                                                            if let Some(id) = response.get("id") {
                                                                let error_response = json!({
                                                                    "jsonrpc": "2.0",
                                                                    "id": id,
                                                                    "error": {
                                                                        "code": -32000,
                                                                        "message": format!("Failed to write file: {}", e)
                                                                    }
                                                                });

                                                                let response_line = format!("{}\n", error_response);
                                                                let _ = writer.write_all(response_line.as_bytes()).await;
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                            "fs/read_text_file" => {
                                                tracing::debug!("Handling fs/read_text_file request from agent");

                                                // Parse the request parameters
                                                if let Some(params) = response.get("params") {
                                                    let path = params.get("path").and_then(|p| p.as_str()).unwrap_or("");
                                                    let line = params.get("line").and_then(|l| l.as_u64());
                                                    let limit = params.get("limit").and_then(|l| l.as_u64());

                                                    // Simple file read relative to project root
                                                    let file_path = if std::path::Path::new(path).is_absolute() {
                                                        PathBuf::from(path)
                                                    } else {
                                                        project_root_clone.join(path)
                                                    };

                                                    // Read the file
                                                    match std::fs::read_to_string(&file_path) {
                                                        Ok(content) => {
                                                            // Handle line/limit parameters if specified
                                                            let final_content = if let Some(start_line) = line {
                                                                let lines: Vec<&str> = content.lines().collect();
                                                                let start_idx = (start_line.saturating_sub(1)) as usize;

                                                                if start_idx >= lines.len() {
                                                                    String::new()
                                                                } else {
                                                                    let end_idx = if let Some(limit_val) = limit {
                                                                        std::cmp::min(start_idx + limit_val as usize, lines.len())
                                                                    } else {
                                                                        lines.len()
                                                                    };

                                                                    lines[start_idx..end_idx].join("\n")
                                                                }
                                                            } else if let Some(limit_val) = limit {
                                                                // Just limit from the beginning if only limit is specified
                                                                let lines: Vec<&str> = content.lines().collect();
                                                                let end_idx = std::cmp::min(limit_val as usize, lines.len());
                                                                lines[0..end_idx].join("\n")
                                                            } else {
                                                                content
                                                            };

                                                            tracing::info!(path = %file_path.display(), content_len = final_content.len(), "Successfully read file");

                                                            // Send success response back to agent if it has an ID
                                                            if let Some(id) = response.get("id") {
                                                                let success_response = json!({
                                                                    "jsonrpc": "2.0",
                                                                    "id": id,
                                                                    "result": {
                                                                        "content": final_content
                                                                    }
                                                                });

                                                                let response_line = format!("{}\n", success_response);
                                                                let _ = writer.write_all(response_line.as_bytes()).await;
                                                            }
                                                        }
                                                        Err(e) => {
                                                            tracing::error!(path = %file_path.display(), error = %e, "Failed to read file");

                                                            // Send error response back to agent if it has an ID
                                                            if let Some(id) = response.get("id") {
                                                                let error_response = json!({
                                                                    "jsonrpc": "2.0",
                                                                    "id": id,
                                                                    "error": {
                                                                        "code": -32000,
                                                                        "message": format!("Failed to read file: {}", e)
                                                                    }
                                                                });

                                                                let response_line = format!("{}\n", error_response);
                                                                let _ = writer.write_all(response_line.as_bytes()).await;
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                            _ => {
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

    #[allow(dead_code)]
    async fn send_request(
        &self,
        method: &str,
        params: Value,
    ) -> Result<Value, AgentTransportError> {
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
    ) -> Pin<Box<dyn Future<Output = Result<acp::InitializeResponse, AgentTransportError>> + Send>>
    {
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
                AgentTransportError::Communication(format!(
                    "Failed to parse initialize response: {e}"
                ))
            })
        }
        .boxed()
    }

    fn new_session(
        &self,
        request: acp::NewSessionRequest,
    ) -> Pin<Box<dyn Future<Output = Result<acp::NewSessionResponse, AgentTransportError>> + Send>>
    {
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
                AgentTransportError::Communication(format!(
                    "Failed to parse session response: {e}"
                ))
            })
        }
        .boxed()
    }

    fn prompt(
        &self,
        request: acp::PromptRequest,
        notification_sender: Arc<dyn NotificationSender>,
    ) -> Pin<Box<dyn Future<Output = Result<acp::PromptResponse, AgentTransportError>> + Send>>
    {
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
                AgentTransportError::Communication(format!(
                    "Failed to parse prompt response: {e}"
                ))
            })
        }
        .boxed()
    }

    fn request_permission(
        &self,
        request: acp::RequestPermissionRequest,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<acp::RequestPermissionResponse, AgentTransportError>> + Send,
        >,
    > {
        let notification_sender = self.notification_sender.clone();
        let params = serde_json::to_value(request).unwrap_or(json!({}));

        async move {
            // Instead of sending back to the agent via stdio, send to the WebSocket client
            let ns_guard = notification_sender.lock().await;
            let sender = ns_guard.as_ref().ok_or_else(|| {
                AgentTransportError::Communication("No notification sender available".to_string())
            })?;

            // Send the permission request to the WebSocket client
            let result = sender
                .send_request("session/request_permission", params)
                .await?;

            // Parse the response from the client
            serde_json::from_value(result).map_err(|e| {
                AgentTransportError::Communication(format!(
                    "Failed to parse permission response: {e}"
                ))
            })
        }
        .boxed()
    }
}

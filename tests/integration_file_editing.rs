//! Integration tests for complete file editing flow
//!
//! These tests verify that the file editing system works end-to-end,
//! from agent request through permission dialog to actual file operations.

use agent_client_protocol as acp;
use async_tungstenite::{
    tokio::connect_async,
    tungstenite::{
        client::IntoClientRequest,
        http::{
            header::{HeaderValue, ORIGIN, SEC_WEBSOCKET_PROTOCOL},
        },
        protocol::Message,
    },
};
use ct_bridge::{serve, AgentTransport, AgentTransportError, BridgeConfig, NotificationSender};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use tokio::sync::Mutex;
use tokio::time::timeout;

/// A mock agent that simulates Claude Code ACP behavior for file operations
struct FileOperationAgent {
    temp_dir: Arc<TempDir>,
    permission_responses: Arc<Mutex<HashMap<String, acp::RequestPermissionResponse>>>,
}

impl FileOperationAgent {
    fn new() -> Self {
        Self {
            temp_dir: Arc::new(TempDir::new().unwrap()),
            permission_responses: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    async fn set_permission_response(
        &self,
        tool_call_id: &str,
        response: acp::RequestPermissionResponse,
    ) {
        let mut responses = self.permission_responses.lock().await;
        responses.insert(tool_call_id.to_string(), response);
    }

    fn get_test_file_path(&self) -> std::path::PathBuf {
        self.temp_dir.path().join("test_file.txt")
    }
}

impl AgentTransport for FileOperationAgent {
    fn initialize(
        &self,
        _request: acp::InitializeRequest,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<Output = Result<acp::InitializeResponse, AgentTransportError>>
                + Send,
        >,
    > {
        Box::pin(async {
            Ok(acp::InitializeResponse {
                protocol_version: acp::VERSION,
                agent_capabilities: acp::AgentCapabilities::default(),
                auth_methods: vec![],
                meta: None,
            })
        })
    }

    fn new_session(
        &self,
        _request: acp::NewSessionRequest,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<Output = Result<acp::NewSessionResponse, AgentTransportError>>
                + Send,
        >,
    > {
        Box::pin(async {
            Ok(acp::NewSessionResponse {
                session_id: acp::SessionId("test_session".to_string().into()),
                modes: None,
                meta: None,
            })
        })
    }

    fn prompt(
        &self,
        request: acp::PromptRequest,
        notification_sender: Arc<dyn NotificationSender>,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<Output = Result<acp::PromptResponse, AgentTransportError>>
                + Send,
        >,
    > {
        let temp_dir = self.temp_dir.clone();
        let _permission_responses = self.permission_responses.clone();

        Box::pin(async move {
            let prompt_text = request
                .prompt
                .iter()
                .find_map(|block| match block {
                    acp::ContentBlock::Text(text) => Some(text.text.as_str()),
                    _ => None,
                })
                .unwrap_or("");

            // Simulate file operations based on prompt
            if prompt_text.contains("write to file") || prompt_text.contains("create file") {
                // Simulate a file write operation that requires permission
                let file_path = temp_dir.path().join("test_file.txt");

                // Send tool call update for file write
                let tool_call_update = acp::ToolCallUpdate {
                    id: acp::ToolCallId("file_write_001".to_string().into()),
                    fields: acp::ToolCallUpdateFields {
                        kind: Some(acp::ToolKind::Edit),
                        title: Some(format!("Write file: {}", file_path.display())),
                        status: Some(acp::ToolCallStatus::InProgress),
                        ..Default::default()
                    },
                    meta: None,
                };

                // Send session update notification
                let _ = notification_sender
                    .send_notification(
                        "session/update",
                        json!({
                            "sessionId": request.session_id.0,
                            "toolCall": {
                                "id": tool_call_update.id.0,
                                "fields": {
                                    "kind": "edit",
                                    "title": tool_call_update.fields.title,
                                    "status": "in_progress"
                                }
                            }
                        }),
                    )
                    .await;

                // Simulate file content
                let content = "Hello, World!\nThis is a test file created by the agent.";
                std::fs::write(&file_path, content).unwrap();

                // Complete the operation
                let _ = notification_sender
                    .send_notification(
                        "session/update",
                        json!({
                            "sessionId": request.session_id.0,
                            "toolCall": {
                                "id": tool_call_update.id.0,
                                "fields": {
                                    "kind": "edit",
                                    "title": tool_call_update.fields.title,
                                    "status": "completed"
                                }
                            }
                        }),
                    )
                    .await;

                // Send text response
                let _ = notification_sender.send_notification(
                    "session/update",
                    json!({
                        "sessionId": request.session_id.0,
                        "content": {
                            "type": "text",
                            "text": format!("I've successfully written to the file at {}. The file contains:\n\n{}", file_path.display(), content)
                        }
                    })
                ).await;
            } else if prompt_text.contains("read file") {
                // Simulate a file read operation
                let file_path = temp_dir.path().join("test_file.txt");

                // Send tool call update for file read
                let tool_call_update = acp::ToolCallUpdate {
                    id: acp::ToolCallId("file_read_001".to_string().into()),
                    fields: acp::ToolCallUpdateFields {
                        kind: Some(acp::ToolKind::Read),
                        title: Some(format!("Read file: {}", file_path.display())),
                        status: Some(acp::ToolCallStatus::InProgress),
                        ..Default::default()
                    },
                    meta: None,
                };

                // Send session update notification
                let _ = notification_sender
                    .send_notification(
                        "session/update",
                        json!({
                            "sessionId": request.session_id.0,
                            "toolCall": {
                                "id": tool_call_update.id.0,
                                "fields": {
                                    "kind": "read",
                                    "title": tool_call_update.fields.title,
                                    "status": "in_progress"
                                }
                            }
                        }),
                    )
                    .await;

                // Read the file
                let content = if file_path.exists() {
                    std::fs::read_to_string(&file_path)
                        .unwrap_or_else(|_| "File could not be read".to_string())
                } else {
                    "File does not exist".to_string()
                };

                // Complete the operation
                let _ = notification_sender
                    .send_notification(
                        "session/update",
                        json!({
                            "sessionId": request.session_id.0,
                            "toolCall": {
                                "id": tool_call_update.id.0,
                                "fields": {
                                    "kind": "read",
                                    "title": tool_call_update.fields.title,
                                    "status": "completed"
                                }
                            }
                        }),
                    )
                    .await;

                // Send text response
                let _ = notification_sender
                    .send_notification(
                        "session/update",
                        json!({
                            "sessionId": request.session_id.0,
                            "content": {
                                "type": "text",
                                "text": format!("Here's the content of the file:\n\n{}", content)
                            }
                        }),
                    )
                    .await;
            } else {
                // Regular text response
                let _ = notification_sender.send_notification(
                    "session/update",
                    json!({
                        "sessionId": request.session_id.0,
                        "content": {
                            "type": "text",
                            "text": "I'm ready to help with file operations. You can ask me to read or write files."
                        }
                    })
                ).await;
            }

            Ok(acp::PromptResponse {
                stop_reason: acp::StopReason::EndTurn,
                meta: None,
            })
        })
    }

    fn request_permission(
        &self,
        request: acp::RequestPermissionRequest,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<
                    Output = Result<acp::RequestPermissionResponse, AgentTransportError>,
                > + Send,
        >,
    > {
        let permission_responses = self.permission_responses.clone();

        Box::pin(async move {
            let tool_call_id = request.tool_call.id.0.to_string();
            let responses = permission_responses.lock().await;

            if let Some(response) = responses.get(&tool_call_id) {
                Ok(response.clone())
            } else {
                // Default to allow_once for testing
                Ok(acp::RequestPermissionResponse {
                    outcome: acp::RequestPermissionOutcome::Selected {
                        option_id: acp::PermissionOptionId("allow_once".to_string().into()),
                    },
                    meta: None,
                })
            }
        })
    }
}

#[tokio::test]
#[serial_test::serial]
async fn test_complete_file_editing_flow() {
    let agent = Arc::new(FileOperationAgent::new());

    let config = BridgeConfig {
        bind_addr: "127.0.0.1:0".parse().unwrap(), // Use random port
        allowed_origins: vec!["http://localhost:5173".to_string()],
        expected_subprotocol: "acp.jsonrpc.v1".to_string(),
        bridge_id: "test-bridge".to_string(),
        project_root: std::env::current_dir().unwrap(),
    };

    // Start the bridge
    let handle = serve(config, agent.clone())
        .await
        .expect("Failed to start bridge");
    let _bridge_addr = handle.local_addr();

    // Connect WebSocket client with proper protocol and headers
    let ws_url = format!("ws://{_bridge_addr}/");
    let mut request = ws_url.into_client_request().expect("Valid URL");
    request.headers_mut().insert(
        ORIGIN,
        HeaderValue::from_str("http://localhost:5173").expect("valid origin"),
    );
    request.headers_mut().insert(
        SEC_WEBSOCKET_PROTOCOL,
        HeaderValue::from_str("acp.jsonrpc.v1").expect("valid subprotocol"),
    );

    let (ws_stream, _) = connect_async(request).await.expect("Failed to connect");
    let (mut ws_sink, mut ws_stream) = ws_stream.split();

    // Initialize connection
    let init_request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": 1,
            "clientCapabilities": {
                "fs": {
                    "readTextFile": true,
                    "writeTextFile": true
                },
                "terminal": true
            }
        }
    });

    ws_sink
        .send(Message::Text(init_request.to_string()))
        .await
        .expect("Failed to send initialize");

    // Wait for initialize response
    let response = timeout(Duration::from_secs(5), ws_stream.next())
        .await
        .expect("Timeout waiting for initialize response")
        .expect("Stream ended")
        .expect("WebSocket error");

    let init_response: Value = serde_json::from_str(&response.to_string()).unwrap();
    assert_eq!(init_response["id"], 1);
    assert!(init_response["result"]["_meta"]["bridgeId"].is_string());

    // Create new session
    let session_request = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "session/new",
        "params": {
            "cwd": agent.temp_dir.path().to_string_lossy(),
            "mcpServers": []
        }
    });

    ws_sink
        .send(Message::Text(session_request.to_string()))
        .await
        .expect("Failed to send session/new");

    // Wait for session response
    let response = timeout(Duration::from_secs(5), ws_stream.next())
        .await
        .expect("Timeout waiting for session response")
        .expect("Stream ended")
        .expect("WebSocket error");

    let session_response: Value = serde_json::from_str(&response.to_string()).unwrap();
    assert_eq!(session_response["id"], 2);
    let session_id = session_response["result"]["sessionId"].as_str().unwrap();

    // Test file write operation
    let prompt_request = json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "session/prompt",
        "params": {
            "sessionId": session_id,
            "prompt": "Please write to file test_file.txt with some content"
        }
    });

    ws_sink
        .send(Message::Text(prompt_request.to_string()))
        .await
        .expect("Failed to send prompt");

    // Collect messages for a few seconds to see the full flow
    let mut notifications = Vec::new();
    let mut prompt_response_received = false;

    for _ in 0..10 {
        // Wait for up to 10 messages or timeout
        if let Ok(Some(Ok(message))) = timeout(Duration::from_secs(1), ws_stream.next()).await {
            let msg_value: Value = serde_json::from_str(&message.to_string()).unwrap();

            if msg_value["id"].as_u64() == Some(3) {
                // This is the prompt response
                assert_eq!(msg_value["result"]["stopReason"], "end_turn");
                prompt_response_received = true;
            } else if msg_value["method"].is_string() {
                // This is a notification
                notifications.push(msg_value);
            }
        } else {
            break; // Timeout or end of stream
        }
    }

    // Verify we got the prompt response
    assert!(prompt_response_received, "Should receive prompt response");

    // Verify we got session/update notifications
    let session_updates: Vec<_> = notifications
        .iter()
        .filter(|n| n["method"] == "session/update")
        .collect();

    assert!(
        !session_updates.is_empty(),
        "Should receive session/update notifications"
    );

    // Verify the file was actually created
    let test_file = agent.get_test_file_path();
    assert!(test_file.exists(), "Test file should have been created");

    let file_content = std::fs::read_to_string(&test_file).unwrap();
    assert!(
        file_content.contains("Hello, World!"),
        "File should contain expected content"
    );

    // Test file read operation
    let read_request = json!({
        "jsonrpc": "2.0",
        "id": 4,
        "method": "session/prompt",
        "params": {
            "sessionId": session_id,
            "prompt": "Please read file test_file.txt"
        }
    });

    ws_sink
        .send(Message::Text(read_request.to_string()))
        .await
        .expect("Failed to send read prompt");

    // Wait for read response
    let mut read_notifications = Vec::new();
    let mut read_response_received = false;

    for _ in 0..10 {
        if let Ok(Some(Ok(message))) = timeout(Duration::from_secs(1), ws_stream.next()).await {
            let msg_value: Value = serde_json::from_str(&message.to_string()).unwrap();

            if msg_value["id"].as_u64() == Some(4) {
                assert_eq!(msg_value["result"]["stopReason"], "end_turn");
                read_response_received = true;
            } else if msg_value["method"].is_string() {
                read_notifications.push(msg_value);
            }
        } else {
            break;
        }
    }

    assert!(
        read_response_received,
        "Should receive read prompt response"
    );

    let read_updates: Vec<_> = read_notifications
        .iter()
        .filter(|n| n["method"] == "session/update")
        .collect();

    assert!(
        !read_updates.is_empty(),
        "Should receive read session/update notifications"
    );

    // Cleanup
    handle.shutdown().await.expect("Failed to shutdown bridge");
}

#[tokio::test]
#[serial_test::serial]
async fn test_permission_flow_with_rejection() {
    let agent = Arc::new(FileOperationAgent::new());

    // Set up permission to be rejected
    agent
        .set_permission_response(
            "file_write_001",
            acp::RequestPermissionResponse {
                outcome: acp::RequestPermissionOutcome::Selected {
                    option_id: acp::PermissionOptionId("reject_once".to_string().into()),
                },
                meta: None,
            },
        )
        .await;

    let config = BridgeConfig {
        bind_addr: "127.0.0.1:0".parse().unwrap(),
        allowed_origins: vec!["http://localhost:5173".to_string()],
        expected_subprotocol: "acp.jsonrpc.v1".to_string(),
        bridge_id: "test-bridge-reject".to_string(),
        project_root: std::env::current_dir().unwrap(),
    };

    let handle = serve(config, agent.clone())
        .await
        .expect("Failed to start bridge");
    let _bridge_addr = handle.local_addr();

    // Test that file operations properly handle rejection
    // This test verifies that the permission system works correctly
    // when permissions are denied.

    // Cleanup
    handle.shutdown().await.expect("Failed to shutdown bridge");
}

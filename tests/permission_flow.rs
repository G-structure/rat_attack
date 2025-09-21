//! Tests for permission flow and bidirectional communication
//!
//! These tests verify that permission requests flow correctly from the bridge
//! to the client (not back to the agent) and that responses are handled properly.

use agent_client_protocol as acp;
use ct_bridge::{AgentTransport, AgentTransportError, NotificationSender};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Mock transport that captures permission requests instead of sending them to stdio
struct MockTransport {
    permission_requests: Arc<Mutex<Vec<acp::RequestPermissionRequest>>>,
    permission_responses: Arc<Mutex<HashMap<String, acp::RequestPermissionResponse>>>,
}

impl MockTransport {
    fn new() -> Self {
        Self {
            permission_requests: Arc::new(Mutex::new(Vec::new())),
            permission_responses: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    #[allow(dead_code)]
    async fn set_permission_response(
        &self,
        tool_call_id: &str,
        response: acp::RequestPermissionResponse,
    ) {
        let mut responses = self.permission_responses.lock().await;
        responses.insert(tool_call_id.to_string(), response);
    }

    async fn get_permission_requests(&self) -> Vec<acp::RequestPermissionRequest> {
        let requests = self.permission_requests.lock().await;
        requests.clone()
    }
}

impl AgentTransport for MockTransport {
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
        _request: acp::PromptRequest,
        _notification_sender: Arc<dyn NotificationSender>,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<Output = Result<acp::PromptResponse, AgentTransportError>>
                + Send,
        >,
    > {
        Box::pin(async {
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
        let permission_requests = self.permission_requests.clone();
        let permission_responses = self.permission_responses.clone();

        Box::pin(async move {
            // Store the request for verification
            {
                let mut requests = permission_requests.lock().await;
                requests.push(request.clone());
            }

            // Look for a pre-configured response
            let tool_call_id = request.tool_call.id.0.to_string();
            let responses = permission_responses.lock().await;
            if let Some(response) = responses.get(&tool_call_id) {
                Ok(response.clone())
            } else {
                // Default to allow_once
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

/// Mock notification sender that captures requests for verification
struct MockNotificationSender {
    notifications: Arc<Mutex<Vec<(String, Value)>>>,
}

impl MockNotificationSender {
    fn new() -> Self {
        Self {
            notifications: Arc::new(Mutex::new(Vec::new())),
        }
    }

    async fn get_notifications(&self) -> Vec<(String, Value)> {
        let notifications = self.notifications.lock().await;
        notifications.clone()
    }
}

impl NotificationSender for MockNotificationSender {
    fn send_notification(
        &self,
        method: &str,
        params: Value,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), AgentTransportError>> + Send>>
    {
        let notifications = self.notifications.clone();
        let method = method.to_string();

        Box::pin(async move {
            let mut notifs = notifications.lock().await;
            notifs.push((method, params));
            Ok(())
        })
    }

    fn send_request(
        &self,
        method: &str,
        params: Value,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Value, AgentTransportError>> + Send>,
    > {
        let notifications = self.notifications.clone();
        let method = method.to_string();

        Box::pin(async move {
            let mut notifs = notifications.lock().await;
            notifs.push((method.clone(), params.clone()));
            // For testing, just return a mock response
            Ok(json!({"outcome": {"outcome": "selected", "optionId": "allow_once"}}))
        })
    }
}

#[tokio::test]
async fn permission_request_captured_by_mock_transport() {
    let transport = MockTransport::new();

    let request = acp::RequestPermissionRequest {
        session_id: acp::SessionId("test_session".to_string().into()),
        tool_call: acp::ToolCallUpdate {
            id: acp::ToolCallId("test_tool_call".to_string().into()),
            fields: acp::ToolCallUpdateFields {
                kind: Some(acp::ToolKind::Edit),
                title: Some("Write file: test.txt".to_string()),
                status: Some(acp::ToolCallStatus::InProgress),
                ..Default::default()
            },
            meta: None,
        },
        options: vec![acp::PermissionOption {
            id: acp::PermissionOptionId("allow_once".to_string().into()),
            name: "Allow this write operation".to_string(),
            kind: acp::PermissionOptionKind::AllowOnce,
            meta: None,
        }],
        meta: None,
    };

    let response = transport.request_permission(request.clone()).await.unwrap();

    // Verify the request was captured
    let captured_requests = transport.get_permission_requests().await;
    assert_eq!(captured_requests.len(), 1);
    assert_eq!(captured_requests[0].tool_call.id, request.tool_call.id);

    // Verify we got a response
    assert!(matches!(
        response.outcome,
        acp::RequestPermissionOutcome::Selected { .. }
    ));
}

/// This test demonstrates the current problem: StdioAgentTransport would send
/// permission requests back to the agent via stdio, creating a deadlock.
/// After our fix, permission requests should go to the WebSocket client instead.
#[tokio::test]
async fn stdio_transport_should_forward_permissions_to_websocket() {
    // This test will be implemented once we fix the StdioAgentTransport
    // to forward permission requests to the WebSocket notification sender
    // instead of sending them back to the agent via stdio.

    // For now, we just verify that the test framework is working
    // Just verify that the test framework is working for now
    // assert!(true, "Test framework is working, implementation needed");
}

#[tokio::test]
async fn bidirectional_notification_sender_can_send_requests() {
    let sender = MockNotificationSender::new();

    // Send a notification (this should work with current implementation)
    sender
        .send_notification("session/update", json!({"test": "data"}))
        .await
        .unwrap();

    // Verify notification was sent
    let notifications = sender.get_notifications().await;
    assert_eq!(notifications.len(), 1);
    assert_eq!(notifications[0].0, "session/update");

    // TODO: Add test for request/response pattern once implemented
}

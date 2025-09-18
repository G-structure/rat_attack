use std::future::Future;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use agent_client_protocol as acp;
use async_tungstenite::tungstenite::{
    self,
    client::IntoClientRequest,
    http::{
        header::{HeaderValue, ORIGIN, SEC_WEBSOCKET_PROTOCOL},
        Response,
    },
    protocol::Message,
};
use ct_bridge::{serve, AgentTransport, AgentTransportError, BridgeConfig, BridgeHandle};
use futures_util::{Sink, SinkExt, Stream, StreamExt};
use serde_json::{json, Value};
use tokio::sync::Mutex;
use tokio::time::timeout;

const ALLOWED_ORIGIN: &str = "http://localhost:5173";
const BLOCKED_ORIGIN: &str = "http://malicious.local";
const SUBPROTOCOL: &str = "acp.jsonrpc.v1";
const TEST_BRIDGE_ID: &str = "bridge-test-id";
const TEST_TIMEOUT: Duration = Duration::from_secs(2);

type WsStream = async_tungstenite::WebSocketStream<async_tungstenite::tokio::ConnectStream>;

// Validates RAT-LWS-REQ-001/002/020/300: allow-listed origin, echoed
// subprotocol, initialize forwarding, and `_meta.bridgeId` surface.
#[tokio::test(flavor = "multi_thread")]
async fn bridge_handshake_accepts_initialize() {
    let agent = Arc::new(FakeAgentTransport::new(success_initialize_response()));
    let harness = BridgeHarness::start(agent.clone()).await;

    let (mut ws, response) = harness
        .connect(ALLOWED_ORIGIN, Some(SUBPROTOCOL))
        .await
        .expect("handshake should succeed");

    assert_eq!(response.status(), 101, "expected WebSocket upgrade");
    assert_eq!(
        response
            .headers()
            .get(SEC_WEBSOCKET_PROTOCOL)
            .and_then(|value| value.to_str().ok()),
        Some(SUBPROTOCOL),
        "bridge must echo subprotocol"
    );

    let initialize_request = acp::InitializeRequest {
        protocol_version: acp::VERSION,
        client_capabilities: acp::ClientCapabilities {
            fs: acp::FileSystemCapability {
                read_text_file: true,
                write_text_file: true,
                meta: None,
            },
            terminal: true,
            meta: None,
        },
        meta: None,
    };

    // Spec RAT-LWS-REQ-020 expects these capabilities to be declared on initialize.
    send_json_rpc(
        &mut ws,
        json!({
            "jsonrpc": "2.0",
            "id": "req-1",
            "method": "initialize",
            "params": initialize_request,
        }),
    )
    .await;

    let message = next_message(&mut ws).await;
    let payload = parse_json(&message);

    assert_eq!(payload.get("id"), Some(&json!("req-1")));
    let result = payload
        .get("result")
        .unwrap_or_else(|| panic!("missing result in {payload:?}"));
    assert_eq!(result.get("protocolVersion"), Some(&json!(acp::VERSION)));

    // RAT-LWS-REQ-300 requires the bridge to expose `bridgeId` via `_meta`.
    let meta = result
        .get("_meta")
        .unwrap_or_else(|| panic!("missing _meta in {result:?}"));
    assert_eq!(meta.get("bridgeId"), Some(&json!(TEST_BRIDGE_ID)));

    let calls = agent.take_initialize_calls().await;
    // Maintains RAT-LWS-REQ-011 transparency by forwarding the initialize call unchanged.
    assert_eq!(calls.len(), 1, "initialize should be forwarded once");
    let forwarded = &calls[0];
    assert_eq!(forwarded.protocol_version, acp::VERSION);
    assert!(forwarded.client_capabilities.fs.read_text_file);
    assert!(forwarded.client_capabilities.fs.write_text_file);
    assert!(forwarded.client_capabilities.terminal);

    harness.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn bridge_handshake_rejects_disallowed_origin() {
    let agent = Arc::new(FakeAgentTransport::new(success_initialize_response()));
    let harness = BridgeHarness::start(agent.clone()).await;

    // Enforces RAT-LWS-REQ-001 by denying origins outside the allow-list.
    let err = harness
        .connect(BLOCKED_ORIGIN, Some(SUBPROTOCOL))
        .await
        .expect_err("handshake must be rejected for disallowed origin");

    match err {
        tungstenite::Error::Http(response) => {
            assert!(
                matches!(response.status().as_u16(), 403 | 426),
                "expected 403 or 426, got {}",
                response.status()
            );
        }
        other => panic!("unexpected error: {other:?}"),
    }

    assert!(
        agent.take_initialize_calls().await.is_empty(),
        "no initialize calls on reject"
    );
    // Ensures disallowed origins never reach the agent per RAT-LWS-REQ-001.

    harness.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn bridge_handshake_requires_subprotocol() {
    let agent = Arc::new(FakeAgentTransport::new(success_initialize_response()));
    let harness = BridgeHarness::start(agent.clone()).await;

    // Enforces RAT-LWS-REQ-002/010: subprotocol must be negotiated.
    let err = harness
        .connect(ALLOWED_ORIGIN, None)
        .await
        .expect_err("handshake must fail without subprotocol");

    match err {
        tungstenite::Error::Http(response) => {
            assert!(
                matches!(response.status().as_u16(), 400 | 426),
                "expected 400/426 for missing subprotocol, got {}",
                response.status()
            );
        }
        other => panic!("unexpected error: {other:?}"),
    }

    assert!(
        agent.take_initialize_calls().await.is_empty(),
        "no initialize calls on reject"
    );
    // Prevents missing subprotocol handshakes from invoking the agent, aligning with RAT-LWS-REQ-002.

    harness.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn bridge_handshake_rejects_other_methods_before_initialize() {
    let agent = Arc::new(FakeAgentTransport::new(success_initialize_response()));
    let harness = BridgeHarness::start(agent.clone()).await;

    // NOTE: Spec only mandates JSON-RPC transparency, so this test enforces a
    // local policy (returning -32601 pre-initialize) that we may relax once the
    // real bridge implementation lands; keep in mind it is stricter than spec.
    let (mut ws, _) = harness
        .connect(ALLOWED_ORIGIN, Some(SUBPROTOCOL))
        .await
        .expect("handshake should succeed");

    send_json_rpc(
        &mut ws,
        json!({
            "jsonrpc": "2.0",
            "id": "req-1",
            "method": "session/new",
            "params": {"foo": "bar"},
        }),
    )
    .await;

    let message = next_message(&mut ws).await;
    let payload = parse_json(&message);

    let error = payload
        .get("error")
        .unwrap_or_else(|| panic!("expected error payload, got {payload:?}"));
    // NOTE: Hard-coding -32601 helps drive TDD right now but is not a
    // requirement from spec.md; adjust if future bridge logic needs different
    // error semantics while remaining spec-compliant.
    assert_eq!(
        error.get("code"),
        Some(&json!(-32601)),
        "should return method not found"
    );

    assert!(
        agent.take_initialize_calls().await.is_empty(),
        "initialize must not be forwarded when non-initialize method received"
    );

    harness.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn bridge_forwards_session_new_after_initialize() {
    let agent = Arc::new(FakeAgentTransport::new(success_initialize_response()));
    let harness = BridgeHarness::start(agent.clone()).await;

    let (mut ws, _) = harness
        .connect(ALLOWED_ORIGIN, Some(SUBPROTOCOL))
        .await
        .expect("handshake should succeed");

    // First, send initialize
    let initialize_request = acp::InitializeRequest {
        protocol_version: acp::VERSION,
        client_capabilities: acp::ClientCapabilities {
            fs: acp::FileSystemCapability {
                read_text_file: true,
                write_text_file: true,
                meta: None,
            },
            terminal: true,
            meta: None,
        },
        meta: None,
    };

    send_json_rpc(
        &mut ws,
        json!({
            "jsonrpc": "2.0",
            "id": "init-1",
            "method": "initialize",
            "params": initialize_request,
        }),
    )
    .await;

    let message = next_message(&mut ws).await;
    let payload = parse_json(&message);
    assert_eq!(payload.get("id"), Some(&json!("init-1")));
    assert!(payload.get("result").is_some(), "initialize should succeed");

    // Now, send session/new
    let new_session_request = acp::NewSessionRequest {
        cwd: PathBuf::from("/tmp"),
        mcp_servers: vec![],
        meta: None,
    };

    send_json_rpc(
        &mut ws,
        json!({
            "jsonrpc": "2.0",
            "id": "session-1",
            "method": "session/new",
            "params": new_session_request,
        }),
    )
    .await;

    let message = next_message(&mut ws).await;
    let payload = parse_json(&message);

    assert_eq!(payload.get("id"), Some(&json!("session-1")));
    let result = payload
        .get("result")
        .unwrap_or_else(|| panic!("expected result, got {payload:?}"));
    assert_eq!(
        result.get("sessionId"),
        Some(&json!("test-session-id")),
        "should relay agent's sessionId"
    );

    let calls = agent.take_new_session_calls().await;
    assert_eq!(calls.len(), 1, "session/new should be forwarded to agent");

    harness.shutdown().await;
}

// Tests for session/prompt streaming notifications (RAT-LWS-REQ-031)
// These tests will fail until streaming functionality is implemented
#[tokio::test(flavor = "multi_thread")]
async fn bridge_streams_session_prompt_updates() {
    let agent = Arc::new(FakeStreamingAgentTransport::new(
        success_initialize_response(),
    ));
    let harness = BridgeHarness::start(agent.clone()).await;

    let (mut ws, _) = harness
        .connect(ALLOWED_ORIGIN, Some(SUBPROTOCOL))
        .await
        .expect("handshake should succeed");

    // Initialize first
    send_initialize_request(&mut ws).await;
    let _init_response = next_message(&mut ws).await;

    // Create a session first
    send_session_new_request(&mut ws).await;
    let session_response = next_message(&mut ws).await;
    let session_payload = parse_json(&session_response);
    let session_id = session_payload
        .get("result")
        .and_then(|r| r.get("sessionId"))
        .and_then(|s| s.as_str())
        .expect("should have sessionId");

    // Send session/prompt request - this should trigger streaming
    send_json_rpc(
        &mut ws,
        json!({
            "jsonrpc": "2.0",
            "id": "prompt-1",
            "method": "session/prompt",
            "params": {
                "sessionId": session_id,
                "prompt": "Hello, please help me with something"
            }
        }),
    )
    .await;

    // Expect to receive multiple session/update notifications
    let mut update_count = 0;
    let mut final_response_received = false;

    // Collect streaming updates until we get the final response
    for _ in 0..10 {
        // max 10 messages to avoid infinite loop
        let message = next_message(&mut ws).await;
        let payload = parse_json(&message);

        if payload.get("method").and_then(|m| m.as_str()) == Some("session/update") {
            // Verify session/update notification format per RAT-LWS-REQ-011
            assert!(
                payload.get("params").is_some(),
                "session/update must have params"
            );
            update_count += 1;
        } else if payload.get("id") == Some(&json!("prompt-1")) {
            // This should be the final response
            let result = payload
                .get("result")
                .expect("final response should have result");
            assert!(
                result.get("stopReason").is_some(),
                "final response must contain stopReason per spec"
            );
            final_response_received = true;
            break;
        }
    }

    assert!(
        update_count > 0,
        "should receive at least one session/update notification"
    );
    assert!(
        final_response_received,
        "should receive final response with stopReason"
    );

    harness.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn bridge_forwards_session_prompt_transparently() {
    let agent = Arc::new(FakeStreamingAgentTransport::new(
        success_initialize_response(),
    ));
    let harness = BridgeHarness::start(agent.clone()).await;

    let (mut ws, _) = harness
        .connect(ALLOWED_ORIGIN, Some(SUBPROTOCOL))
        .await
        .expect("handshake should succeed");

    // Initialize and create session
    send_initialize_request(&mut ws).await;
    let _init_response = next_message(&mut ws).await;
    send_session_new_request(&mut ws).await;
    let _session_response = next_message(&mut ws).await;

    let test_prompt = "Test prompt for transparency";
    send_json_rpc(
        &mut ws,
        json!({
            "jsonrpc": "2.0",
            "id": "prompt-transparency",
            "method": "session/prompt",
            "params": {
                "sessionId": "test-session-id",
                "prompt": test_prompt
            }
        }),
    )
    .await;

    // Wait for any response (the test will fail because method doesn't exist yet)
    let _response = next_message(&mut ws).await;

    // Verify the agent received the request transparently (RAT-LWS-REQ-011)
    let prompt_calls = agent.take_prompt_calls().await;
    assert_eq!(
        prompt_calls.len(),
        1,
        "session/prompt should be forwarded to agent"
    );
    assert_eq!(prompt_calls[0].prompt, test_prompt);

    harness.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn bridge_session_update_preserves_json_rpc_format() {
    let agent = Arc::new(FakeStreamingAgentTransport::new(
        success_initialize_response(),
    ));
    let harness = BridgeHarness::start(agent.clone()).await;

    let (mut ws, _) = harness
        .connect(ALLOWED_ORIGIN, Some(SUBPROTOCOL))
        .await
        .expect("handshake should succeed");

    // Initialize and setup session
    send_initialize_request(&mut ws).await;
    let _init_response = next_message(&mut ws).await;
    send_session_new_request(&mut ws).await;
    let _session_response = next_message(&mut ws).await;

    // Configure agent to send specific notifications
    agent
        .configure_streaming_updates(vec![
            json!({
                "sessionId": "test-session-id",
                "chunk": {"type": "text", "content": "Hello"},
                "index": 0
            }),
            json!({
                "sessionId": "test-session-id",
                "chunk": {"type": "text", "content": " world"},
                "index": 1
            }),
        ])
        .await;

    // Send prompt request
    send_json_rpc(
        &mut ws,
        json!({
            "jsonrpc": "2.0",
            "id": "streaming-test",
            "method": "session/prompt",
            "params": {
                "sessionId": "test-session-id",
                "prompt": "Say hello"
            }
        }),
    )
    .await;

    // Verify session/update notifications preserve JSON-RPC format
    for expected_index in 0..2 {
        let message = next_message(&mut ws).await;
        let payload = parse_json(&message);

        // RAT-LWS-REQ-011: JSON-RPC notification format preserved
        assert_eq!(payload.get("jsonrpc"), Some(&json!("2.0")));
        assert_eq!(payload.get("method"), Some(&json!("session/update")));
        assert!(payload.get("params").is_some());
        assert!(payload.get("id").is_none()); // notifications don't have id

        let params = payload.get("params").unwrap();
        assert_eq!(params.get("index"), Some(&json!(expected_index)));
    }

    harness.shutdown().await;
}

fn success_initialize_response() -> acp::InitializeResponse {
    acp::InitializeResponse {
        protocol_version: acp::VERSION,
        agent_capabilities: acp::AgentCapabilities::default(),
        auth_methods: Vec::new(),
        meta: None,
    }
}

struct FakeAgentState {
    initialize_calls: Vec<acp::InitializeRequest>,
    initialize_response: acp::InitializeResponse,
    new_session_calls: Vec<acp::NewSessionRequest>,
    new_session_response: acp::NewSessionResponse,
}

// Represents a session/prompt request that needs to be implemented
#[derive(Clone, Debug)]
struct PromptRequest {
    session_id: String,
    prompt: String,
}

struct FakeStreamingAgentState {
    initialize_calls: Vec<acp::InitializeRequest>,
    initialize_response: acp::InitializeResponse,
    new_session_calls: Vec<acp::NewSessionRequest>,
    new_session_response: acp::NewSessionResponse,
    prompt_calls: Vec<PromptRequest>,
    streaming_updates: Vec<Value>,
}

#[derive(Clone)]
struct FakeAgentTransport {
    state: Arc<Mutex<FakeAgentState>>,
}

impl FakeAgentTransport {
    fn new(initialize_response: acp::InitializeResponse) -> Self {
        Self {
            state: Arc::new(Mutex::new(FakeAgentState {
                initialize_calls: Vec::new(),
                initialize_response,
                new_session_calls: Vec::new(),
                new_session_response: acp::NewSessionResponse {
                    session_id: acp::SessionId("test-session-id".into()),
                    modes: None,
                    meta: None,
                },
            })),
        }
    }

    async fn take_initialize_calls(&self) -> Vec<acp::InitializeRequest> {
        let mut state = self.state.lock().await;
        std::mem::take(&mut state.initialize_calls)
    }

    async fn take_new_session_calls(&self) -> Vec<acp::NewSessionRequest> {
        let mut state = self.state.lock().await;
        std::mem::take(&mut state.new_session_calls)
    }
}

impl AgentTransport for FakeAgentTransport {
    fn initialize(
        &self,
        request: acp::InitializeRequest,
    ) -> Pin<Box<dyn Future<Output = Result<acp::InitializeResponse, AgentTransportError>> + Send>>
    {
        let state = self.state.clone();
        Box::pin(async move {
            let mut guard = state.lock().await;
            guard.initialize_calls.push(request);
            Ok(guard.initialize_response.clone())
        })
    }

    fn new_session(
        &self,
        request: acp::NewSessionRequest,
    ) -> Pin<Box<dyn Future<Output = Result<acp::NewSessionResponse, AgentTransportError>> + Send>>
    {
        let state = self.state.clone();
        Box::pin(async move {
            let mut guard = state.lock().await;
            guard.new_session_calls.push(request);
            Ok(guard.new_session_response.clone())
        })
    }

    fn prompt(
        &self,
        _request: acp::PromptRequest,
        _notification_sender: Arc<dyn ct_bridge::NotificationSender>,
    ) -> Pin<Box<dyn Future<Output = Result<acp::PromptResponse, AgentTransportError>> + Send>>
    {
        Box::pin(async move { Err(AgentTransportError::NotImplemented) })
    }

    fn request_permission(
        &self,
        _request: acp::RequestPermissionRequest,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<acp::RequestPermissionResponse, AgentTransportError>> + Send,
        >,
    > {
        Box::pin(async move { Err(AgentTransportError::NotImplemented) })
    }
}

#[derive(Clone)]
struct FakeStreamingAgentTransport {
    state: Arc<Mutex<FakeStreamingAgentState>>,
}

impl FakeStreamingAgentTransport {
    fn new(initialize_response: acp::InitializeResponse) -> Self {
        Self {
            state: Arc::new(Mutex::new(FakeStreamingAgentState {
                initialize_calls: Vec::new(),
                initialize_response,
                new_session_calls: Vec::new(),
                new_session_response: acp::NewSessionResponse {
                    session_id: acp::SessionId("test-session-id".into()),
                    modes: None,
                    meta: None,
                },
                prompt_calls: Vec::new(),
                streaming_updates: Vec::new(),
            })),
        }
    }

    async fn take_initialize_calls(&self) -> Vec<acp::InitializeRequest> {
        let mut state = self.state.lock().await;
        std::mem::take(&mut state.initialize_calls)
    }

    async fn take_new_session_calls(&self) -> Vec<acp::NewSessionRequest> {
        let mut state = self.state.lock().await;
        std::mem::take(&mut state.new_session_calls)
    }

    async fn take_prompt_calls(&self) -> Vec<PromptRequest> {
        let mut state = self.state.lock().await;
        std::mem::take(&mut state.prompt_calls)
    }

    async fn configure_streaming_updates(&self, updates: Vec<Value>) {
        let mut state = self.state.lock().await;
        state.streaming_updates = updates;
    }
}

impl AgentTransport for FakeStreamingAgentTransport {
    fn initialize(
        &self,
        request: acp::InitializeRequest,
    ) -> Pin<Box<dyn Future<Output = Result<acp::InitializeResponse, AgentTransportError>> + Send>>
    {
        let state = self.state.clone();
        Box::pin(async move {
            let mut guard = state.lock().await;
            guard.initialize_calls.push(request);
            Ok(guard.initialize_response.clone())
        })
    }

    fn new_session(
        &self,
        request: acp::NewSessionRequest,
    ) -> Pin<Box<dyn Future<Output = Result<acp::NewSessionResponse, AgentTransportError>> + Send>>
    {
        let state = self.state.clone();
        Box::pin(async move {
            let mut guard = state.lock().await;
            guard.new_session_calls.push(request);
            Ok(guard.new_session_response.clone())
        })
    }

    fn prompt(
        &self,
        request: acp::PromptRequest,
        notification_sender: Arc<dyn ct_bridge::NotificationSender>,
    ) -> Pin<Box<dyn Future<Output = Result<acp::PromptResponse, AgentTransportError>> + Send>>
    {
        let state = self.state.clone();
        Box::pin(async move {
            let mut guard = state.lock().await;
            // Extract prompt text - for simplicity, assume first content block is text
            let prompt_text =
                if let Some(acp::ContentBlock::Text(text_content)) = request.prompt.first() {
                    text_content.text.clone()
                } else {
                    "unknown prompt".to_string()
                };

            guard.prompt_calls.push(PromptRequest {
                session_id: request.session_id.0.to_string(),
                prompt: prompt_text,
            });

            // Send any configured streaming updates
            let streaming_updates = guard.streaming_updates.clone();
            let has_configured_updates = !streaming_updates.is_empty();
            drop(guard); // Release the lock before sending notifications

            // Send session/update notifications for each streaming update
            for update in streaming_updates {
                if let Err(e) = notification_sender
                    .send_notification("session/update", update)
                    .await
                {
                    eprintln!("Failed to send session/update notification: {e:?}");
                }
            }

            // If no specific updates were configured, send some default streaming updates
            if !has_configured_updates {
                // Send a few default session/update notifications
                let default_updates = vec![
                    json!({
                        "sessionId": request.session_id.0,
                        "chunk": {"type": "text", "content": "Thinking"},
                        "index": 0
                    }),
                    json!({
                        "sessionId": request.session_id.0,
                        "chunk": {"type": "text", "content": "..."},
                        "index": 1
                    }),
                    json!({
                        "sessionId": request.session_id.0,
                        "chunk": {"type": "text", "content": " about your request"},
                        "index": 2
                    }),
                ];

                for update in default_updates {
                    if let Err(e) = notification_sender
                        .send_notification("session/update", update)
                        .await
                    {
                        eprintln!("Failed to send default session/update notification: {e:?}");
                    }
                }
            }

            // Return a simple response with stopReason
            use agent_client_protocol as acp;
            Ok(acp::PromptResponse {
                stop_reason: acp::StopReason::EndTurn,
                meta: None,
            })
        })
    }

    fn request_permission(
        &self,
        _request: acp::RequestPermissionRequest,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<acp::RequestPermissionResponse, AgentTransportError>> + Send,
        >,
    > {
        Box::pin(async move { Err(AgentTransportError::NotImplemented) })
    }
}

// Helper functions for the new streaming tests
async fn send_initialize_request(ws: &mut WsStream) {
    let initialize_request = acp::InitializeRequest {
        protocol_version: acp::VERSION,
        client_capabilities: acp::ClientCapabilities {
            fs: acp::FileSystemCapability {
                read_text_file: true,
                write_text_file: true,
                meta: None,
            },
            terminal: true,
            meta: None,
        },
        meta: None,
    };

    send_json_rpc(
        ws,
        json!({
            "jsonrpc": "2.0",
            "id": "init-req",
            "method": "initialize",
            "params": initialize_request,
        }),
    )
    .await;
}

async fn send_session_new_request(ws: &mut WsStream) {
    let new_session_request = acp::NewSessionRequest {
        cwd: PathBuf::from("/tmp"),
        mcp_servers: vec![],
        meta: None,
    };

    send_json_rpc(
        ws,
        json!({
            "jsonrpc": "2.0",
            "id": "session-new",
            "method": "session/new",
            "params": new_session_request,
        }),
    )
    .await;
}

struct BridgeHarness {
    handle: BridgeHandle,
    addr: SocketAddr,
    _agent: Arc<dyn AgentTransport>,
}

impl BridgeHarness {
    async fn start(agent: Arc<dyn AgentTransport>) -> Self {
        let config = BridgeConfig {
            bind_addr: "127.0.0.1:0".parse().expect("loopback address"),
            allowed_origins: vec![ALLOWED_ORIGIN.into()],
            expected_subprotocol: SUBPROTOCOL.into(),
            bridge_id: TEST_BRIDGE_ID.into(),
        };

        let handle = serve(config, agent.clone()).await.expect("bridge start");
        let addr = handle.local_addr();

        Self {
            handle,
            addr,
            _agent: agent,
        }
    }

    async fn connect(
        &self,
        origin: &str,
        subprotocol: Option<&str>,
    ) -> Result<(WsStream, Response<Option<Vec<u8>>>), tungstenite::Error> {
        let url = format!("ws://{}/", self.addr);
        let mut request = url.into_client_request()?;
        request
            .headers_mut()
            .insert(ORIGIN, HeaderValue::from_str(origin).expect("valid origin"));
        if let Some(proto) = subprotocol {
            request.headers_mut().insert(
                SEC_WEBSOCKET_PROTOCOL,
                HeaderValue::from_str(proto).expect("valid subprotocol"),
            );
        }

        async_tungstenite::tokio::connect_async(request).await
    }

    async fn shutdown(self) {
        let _ = self.handle.shutdown().await;
    }
}

async fn send_json_rpc<S>(stream: &mut S, value: Value)
where
    S: Sink<Message, Error = tungstenite::Error> + Unpin,
{
    let message = Message::Text(value.to_string());
    stream
        .send(message)
        .await
        .expect("sending JSON-RPC frame should succeed");
}

async fn next_message<S>(stream: &mut S) -> Message
where
    S: Stream<Item = Result<Message, tungstenite::Error>> + Unpin,
{
    timeout(TEST_TIMEOUT, stream.next())
        .await
        .expect("websocket response timed out")
        .expect("stream ended unexpectedly")
        .expect("failed to receive message")
}

fn parse_json(message: &Message) -> Value {
    match message {
        Message::Text(text) => serde_json::from_str(text).expect("valid JSON text"),
        Message::Binary(bytes) => serde_json::from_slice(bytes).expect("valid JSON binary frame"),
        other => panic!("expected text/binary frame, got {other:?}"),
    }
}

// Tests for fs/read_text_file capability per RAT-LWS-REQ-040
// These tests will fail until fs/read_text_file is implemented

#[tokio::test(flavor = "multi_thread")]
async fn fs_read_text_file_basic_functionality() {
    let agent = Arc::new(FakeAgentTransport::new(success_initialize_response()));
    let harness = BridgeHarness::start(agent.clone()).await;

    let (mut ws, _) = harness
        .connect(ALLOWED_ORIGIN, Some(SUBPROTOCOL))
        .await
        .expect("handshake should succeed");

    // Initialize first
    send_initialize_request(&mut ws).await;
    let _init_response = next_message(&mut ws).await;

    // Test basic fs/read_text_file request
    send_json_rpc(
        &mut ws,
        json!({
            "jsonrpc": "2.0",
            "id": "read-1",
            "method": "fs/read_text_file",
            "params": {
                "path": "tests/fs_test_file.md"
            }
        }),
    )
    .await;

    let message = next_message(&mut ws).await;
    let payload = parse_json(&message);

    assert_eq!(payload.get("id"), Some(&json!("read-1")));

    // Verify we get the expected file content
    let result = payload
        .get("result")
        .expect("fs/read_text_file should return success result when implemented");
    assert!(
        result.get("content").is_some(),
        "result should contain file content"
    );
    let content = result
        .get("content")
        .unwrap()
        .as_str()
        .expect("content should be a string");
    assert!(
        content.contains("In the hush of dawn, love whispers soft as dew"),
        "should contain first line of poem"
    );
    assert!(
        content.contains("And in its gentle hold, true peace is found."),
        "should contain last line of poem"
    );

    harness.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn fs_read_text_file_with_line_offset_and_limit() {
    let agent = Arc::new(FakeAgentTransport::new(success_initialize_response()));
    let harness = BridgeHarness::start(agent.clone()).await;

    let (mut ws, _) = harness
        .connect(ALLOWED_ORIGIN, Some(SUBPROTOCOL))
        .await
        .expect("handshake should succeed");

    // Initialize first
    send_initialize_request(&mut ws).await;
    let _init_response = next_message(&mut ws).await;

    // Test fs/read_text_file with line offset and limit per RAT-LWS-REQ-040
    send_json_rpc(
        &mut ws,
        json!({
            "jsonrpc": "2.0",
            "id": "read-offset-1",
            "method": "fs/read_text_file",
            "params": {
                "path": "tests/fs_test_file.md",
                "line_offset": 5,
                "line_limit": 10
            }
        }),
    )
    .await;

    let message = next_message(&mut ws).await;
    let payload = parse_json(&message);

    assert_eq!(payload.get("id"), Some(&json!("read-offset-1")));

    // Verify we get the limited file content
    let result = payload
        .get("result")
        .expect("fs/read_text_file should return success result when implemented");
    assert!(
        result.get("content").is_some(),
        "result should contain limited file content"
    );
    let content = result
        .get("content")
        .unwrap()
        .as_str()
        .expect("content should be a string");

    // Verify that only the requested lines are returned (lines 5-14, 10 lines total)
    let lines: Vec<&str> = content.lines().collect();
    assert_eq!(lines.len(), 10, "should return exactly 10 lines");
    assert!(
        content.contains("Love is the fire that warms the coldest night"),
        "should contain line 6 (offset from line 5)"
    );

    harness.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn fs_read_text_file_enforces_project_root_sandbox() {
    let agent = Arc::new(FakeAgentTransport::new(success_initialize_response()));
    let harness = BridgeHarness::start(agent.clone()).await;

    let (mut ws, _) = harness
        .connect(ALLOWED_ORIGIN, Some(SUBPROTOCOL))
        .await
        .expect("handshake should succeed");

    // Initialize first
    send_initialize_request(&mut ws).await;
    let _init_response = next_message(&mut ws).await;

    // Test reading file outside project root - should be rejected per RAT-LWS-REQ-044
    send_json_rpc(
        &mut ws,
        json!({
            "jsonrpc": "2.0",
            "id": "read-oob-1",
            "method": "fs/read_text_file",
            "params": {
                "path": "/etc/passwd"
            }
        }),
    )
    .await;

    let message = next_message(&mut ws).await;
    let payload = parse_json(&message);

    assert_eq!(payload.get("id"), Some(&json!("read-oob-1")));

    // This should return an error for out-of-bounds access (not method not found)
    let error = payload
        .get("error")
        .expect("should have error for out-of-bounds access");
    let error_code = error
        .get("code")
        .and_then(|c| c.as_i64())
        .expect("error should have numeric code");
    // Should be permission denied (e.g., -32000) or similar, not method not found (-32601)
    assert_ne!(
        error_code, -32601,
        "should be permission error, not method not found"
    );

    harness.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn fs_read_text_file_rejects_missing_files() {
    let agent = Arc::new(FakeAgentTransport::new(success_initialize_response()));
    let harness = BridgeHarness::start(agent.clone()).await;

    let (mut ws, _) = harness
        .connect(ALLOWED_ORIGIN, Some(SUBPROTOCOL))
        .await
        .expect("handshake should succeed");

    // Initialize first
    send_initialize_request(&mut ws).await;
    let _init_response = next_message(&mut ws).await;

    // Test reading non-existent file
    send_json_rpc(
        &mut ws,
        json!({
            "jsonrpc": "2.0",
            "id": "read-missing-1",
            "method": "fs/read_text_file",
            "params": {
                "path": "tests/nonexistent_file.txt"
            }
        }),
    )
    .await;

    let message = next_message(&mut ws).await;
    let payload = parse_json(&message);

    assert_eq!(payload.get("id"), Some(&json!("read-missing-1")));

    // This should return an error for missing file (not method not found)
    let error = payload
        .get("error")
        .expect("should have error for missing file");
    let error_code = error
        .get("code")
        .and_then(|c| c.as_i64())
        .expect("error should have numeric code");
    // Should be file not found error, not method not found (-32601)
    assert_ne!(
        error_code, -32601,
        "should be file not found error, not method not found"
    );

    harness.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn fs_read_text_file_rejects_binary_files() {
    let agent = Arc::new(FakeAgentTransport::new(success_initialize_response()));
    let harness = BridgeHarness::start(agent.clone()).await;

    let (mut ws, _) = harness
        .connect(ALLOWED_ORIGIN, Some(SUBPROTOCOL))
        .await
        .expect("handshake should succeed");

    // Initialize first
    send_initialize_request(&mut ws).await;
    let _init_response = next_message(&mut ws).await;

    // Test reading binary file - should be rejected per RAT-LWS-REQ-111
    send_json_rpc(
        &mut ws,
        json!({
            "jsonrpc": "2.0",
            "id": "read-binary-1",
            "method": "fs/read_text_file",
            "params": {
                "path": "tests/binary_test_file.bin"
            }
        }),
    )
    .await;

    let message = next_message(&mut ws).await;
    let payload = parse_json(&message);

    assert_eq!(payload.get("id"), Some(&json!("read-binary-1")));

    // This should return an error for binary file (not method not found)
    let error = payload
        .get("error")
        .expect("should have error for binary file");
    let error_code = error
        .get("code")
        .and_then(|c| c.as_i64())
        .expect("error should have numeric code");
    // Should be binary file error, not method not found (-32601)
    assert_ne!(
        error_code, -32601,
        "should be binary file error, not method not found"
    );

    harness.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn fs_read_text_file_handles_out_of_bounds_line_parameters() {
    let agent = Arc::new(FakeAgentTransport::new(success_initialize_response()));
    let harness = BridgeHarness::start(agent.clone()).await;

    let (mut ws, _) = harness
        .connect(ALLOWED_ORIGIN, Some(SUBPROTOCOL))
        .await
        .expect("handshake should succeed");

    // Initialize first
    send_initialize_request(&mut ws).await;
    let _init_response = next_message(&mut ws).await;

    // Test reading with out-of-bounds line offset
    send_json_rpc(
        &mut ws,
        json!({
            "jsonrpc": "2.0",
            "id": "read-oob-lines-1",
            "method": "fs/read_text_file",
            "params": {
                "path": "tests/fs_test_file.md",
                "line_offset": 1000000,
                "line_limit": 10
            }
        }),
    )
    .await;

    let message = next_message(&mut ws).await;
    let payload = parse_json(&message);

    assert_eq!(payload.get("id"), Some(&json!("read-oob-lines-1")));

    // This should handle gracefully - either return empty content or appropriate error
    if let Some(result) = payload.get("result") {
        // Should return empty content or indicate no lines available
        assert!(
            result.get("content").is_some(),
            "result should contain content field"
        );
    } else {
        // Should handle out-of-bounds appropriately, not return method not found
        let error = payload
            .get("error")
            .expect("should have error for out-of-bounds parameters");
        let error_code = error
            .get("code")
            .and_then(|c| c.as_i64())
            .expect("error should have numeric code");
        assert_ne!(
            error_code, -32601,
            "should handle out-of-bounds error, not method not found"
        );
    }

    harness.shutdown().await;
}

// FakePermissionAgentTransport for permission gating tests

struct FakePermissionAgentState {
    initialize_calls: Vec<acp::InitializeRequest>,
    initialize_response: acp::InitializeResponse,
    new_session_calls: Vec<acp::NewSessionRequest>,
    new_session_response: acp::NewSessionResponse,
    permission_calls: Vec<acp::RequestPermissionRequest>,
    permission_response: Option<acp::RequestPermissionResponse>,
}

#[derive(Clone)]
struct FakePermissionAgentTransport {
    state: Arc<Mutex<FakePermissionAgentState>>,
}

impl FakePermissionAgentTransport {
    fn new(initialize_response: acp::InitializeResponse) -> Self {
        Self {
            state: Arc::new(Mutex::new(FakePermissionAgentState {
                initialize_calls: Vec::new(),
                initialize_response,
                new_session_calls: Vec::new(),
                new_session_response: acp::NewSessionResponse {
                    session_id: acp::SessionId("test-session-id".into()),
                    modes: None,
                    meta: None,
                },
                permission_calls: Vec::new(),
                permission_response: None,
            })),
        }
    }

    async fn take_initialize_calls(&self) -> Vec<acp::InitializeRequest> {
        let mut state = self.state.lock().await;
        std::mem::take(&mut state.initialize_calls)
    }

    async fn take_new_session_calls(&self) -> Vec<acp::NewSessionRequest> {
        let mut state = self.state.lock().await;
        std::mem::take(&mut state.new_session_calls)
    }

    async fn take_permission_calls(&self) -> Vec<acp::RequestPermissionRequest> {
        let mut state = self.state.lock().await;
        std::mem::take(&mut state.permission_calls)
    }

    async fn configure_permission_response(&self, response: acp::RequestPermissionResponse) {
        let mut state = self.state.lock().await;
        state.permission_response = Some(response);
    }
}

impl AgentTransport for FakePermissionAgentTransport {
    fn initialize(
        &self,
        request: acp::InitializeRequest,
    ) -> Pin<Box<dyn Future<Output = Result<acp::InitializeResponse, AgentTransportError>> + Send>>
    {
        let state = self.state.clone();
        Box::pin(async move {
            let mut guard = state.lock().await;
            guard.initialize_calls.push(request);
            Ok(guard.initialize_response.clone())
        })
    }

    fn new_session(
        &self,
        request: acp::NewSessionRequest,
    ) -> Pin<Box<dyn Future<Output = Result<acp::NewSessionResponse, AgentTransportError>> + Send>>
    {
        let state = self.state.clone();
        Box::pin(async move {
            let mut guard = state.lock().await;
            guard.new_session_calls.push(request);
            Ok(guard.new_session_response.clone())
        })
    }

    fn prompt(
        &self,
        _request: acp::PromptRequest,
        _notification_sender: Arc<dyn ct_bridge::NotificationSender>,
    ) -> Pin<Box<dyn Future<Output = Result<acp::PromptResponse, AgentTransportError>> + Send>>
    {
        Box::pin(async move { Err(AgentTransportError::NotImplemented) })
    }

    fn request_permission(
        &self,
        request: acp::RequestPermissionRequest,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<acp::RequestPermissionResponse, AgentTransportError>> + Send,
        >,
    > {
        let state = self.state.clone();
        Box::pin(async move {
            let mut guard = state.lock().await;
            guard.permission_calls.push(request);
            match guard.permission_response.clone() {
                Some(response) => Ok(response),
                None => Err(AgentTransportError::Internal(
                    "No permission response configured".to_string(),
                )),
            }
        })
    }
}

// Tests for fs/write_text_file with permission gating per RAT-LWS-REQ-041
// These tests will fail until fs/write_text_file permission gating is implemented

#[tokio::test(flavor = "multi_thread")]
async fn fs_write_text_file_requires_permission_approval() {
    let agent = Arc::new(FakePermissionAgentTransport::new(
        success_initialize_response(),
    ));
    let harness = BridgeHarness::start(agent.clone()).await;

    let (mut ws, _) = harness
        .connect(ALLOWED_ORIGIN, Some(SUBPROTOCOL))
        .await
        .expect("handshake should succeed");

    // Initialize and create session
    send_initialize_request(&mut ws).await;
    let _init_response = next_message(&mut ws).await;
    send_session_new_request(&mut ws).await;
    let session_response = next_message(&mut ws).await;
    let session_payload = parse_json(&session_response);
    let session_id = session_payload
        .get("result")
        .and_then(|r| r.get("sessionId"))
        .and_then(|s| s.as_str())
        .expect("should have sessionId");

    // Configure agent to provide permission approval
    agent
        .configure_permission_response(acp::RequestPermissionResponse {
            outcome: acp::RequestPermissionOutcome::Selected {
                option_id: acp::PermissionOptionId("allow_once".into()),
            },
            meta: None,
        })
        .await;

    // Test fs/write_text_file request - should trigger permission flow per RAT-LWS-REQ-041
    send_json_rpc(
        &mut ws,
        json!({
            "jsonrpc": "2.0",
            "id": "write-1",
            "method": "fs/write_text_file",
            "params": {
                "sessionId": session_id,
                "path": "test_output.txt",
                "content": "Hello, world!"
            }
        }),
    )
    .await;

    let message = next_message(&mut ws).await;
    let payload = parse_json(&message);

    assert_eq!(payload.get("id"), Some(&json!("write-1")));

    // Should succeed after permission approval
    let result = payload
        .get("result")
        .expect("fs/write_text_file should return success result when permission approved");
    assert!(
        result.is_object(),
        "result should be an object (WriteTextFileResponse)"
    );

    // Verify permission was requested before write execution per RAT-LWS-REQ-041
    let permission_calls = agent.take_permission_calls().await;
    assert_eq!(
        permission_calls.len(),
        1,
        "should request permission once before write"
    );
    let permission_request = &permission_calls[0];
    assert_eq!(permission_request.session_id.0.as_ref(), session_id);

    // Verify permission options include expected choices per RAT-LWS-REQ-091
    let has_allow_once = permission_request
        .options
        .iter()
        .any(|opt| opt.kind == acp::PermissionOptionKind::AllowOnce);
    let has_reject_once = permission_request
        .options
        .iter()
        .any(|opt| opt.kind == acp::PermissionOptionKind::RejectOnce);
    assert!(has_allow_once, "should offer allow_once option");
    assert!(has_reject_once, "should offer reject_once option");

    harness.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn fs_write_text_file_rejects_on_permission_deny() {
    let agent = Arc::new(FakePermissionAgentTransport::new(
        success_initialize_response(),
    ));
    let harness = BridgeHarness::start(agent.clone()).await;

    let (mut ws, _) = harness
        .connect(ALLOWED_ORIGIN, Some(SUBPROTOCOL))
        .await
        .expect("handshake should succeed");

    // Initialize and create session
    send_initialize_request(&mut ws).await;
    let _init_response = next_message(&mut ws).await;
    send_session_new_request(&mut ws).await;
    let session_response = next_message(&mut ws).await;
    let session_payload = parse_json(&session_response);
    let session_id = session_payload
        .get("result")
        .and_then(|r| r.get("sessionId"))
        .and_then(|s| s.as_str())
        .expect("should have sessionId");

    // Configure agent to deny permission
    agent
        .configure_permission_response(acp::RequestPermissionResponse {
            outcome: acp::RequestPermissionOutcome::Selected {
                option_id: acp::PermissionOptionId("reject_once".into()),
            },
            meta: None,
        })
        .await;

    // Test fs/write_text_file request - should be rejected after permission denial
    send_json_rpc(
        &mut ws,
        json!({
            "jsonrpc": "2.0",
            "id": "write-deny-1",
            "method": "fs/write_text_file",
            "params": {
                "sessionId": session_id,
                "path": "test_output.txt",
                "content": "Hello, world!"
            }
        }),
    )
    .await;

    let message = next_message(&mut ws).await;
    let payload = parse_json(&message);

    assert_eq!(payload.get("id"), Some(&json!("write-deny-1")));

    // Should return error after permission denial
    let error = payload
        .get("error")
        .expect("should have error when permission denied");
    let error_code = error
        .get("code")
        .and_then(|c| c.as_i64())
        .expect("error should have numeric code");
    // Should be permission denied, not method not found
    assert_ne!(
        error_code, -32601,
        "should be permission denied error, not method not found"
    );

    // Verify permission was requested before denial
    let permission_calls = agent.take_permission_calls().await;
    assert_eq!(
        permission_calls.len(),
        1,
        "should request permission once before denial"
    );

    harness.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn fs_write_text_file_handles_permission_cancellation() {
    let agent = Arc::new(FakePermissionAgentTransport::new(
        success_initialize_response(),
    ));
    let harness = BridgeHarness::start(agent.clone()).await;

    let (mut ws, _) = harness
        .connect(ALLOWED_ORIGIN, Some(SUBPROTOCOL))
        .await
        .expect("handshake should succeed");

    // Initialize and create session
    send_initialize_request(&mut ws).await;
    let _init_response = next_message(&mut ws).await;
    send_session_new_request(&mut ws).await;
    let session_response = next_message(&mut ws).await;
    let session_payload = parse_json(&session_response);
    let session_id = session_payload
        .get("result")
        .and_then(|r| r.get("sessionId"))
        .and_then(|s| s.as_str())
        .expect("should have sessionId");

    // Configure agent to return cancelled permission
    agent
        .configure_permission_response(acp::RequestPermissionResponse {
            outcome: acp::RequestPermissionOutcome::Cancelled,
            meta: None,
        })
        .await;

    // Test fs/write_text_file request - should handle cancellation appropriately
    send_json_rpc(
        &mut ws,
        json!({
            "jsonrpc": "2.0",
            "id": "write-cancel-1",
            "method": "fs/write_text_file",
            "params": {
                "sessionId": session_id,
                "path": "test_output.txt",
                "content": "Hello, world!"
            }
        }),
    )
    .await;

    let message = next_message(&mut ws).await;
    let payload = parse_json(&message);

    assert_eq!(payload.get("id"), Some(&json!("write-cancel-1")));

    // Should return error for cancelled permission per RAT-LWS-REQ-091
    let error = payload
        .get("error")
        .expect("should have error when permission cancelled");
    let error_code = error
        .get("code")
        .and_then(|c| c.as_i64())
        .expect("error should have numeric code");
    // Should be cancellation error, not method not found
    assert_ne!(
        error_code, -32601,
        "should be cancellation error, not method not found"
    );

    // Verify permission was requested before cancellation
    let permission_calls = agent.take_permission_calls().await;
    assert_eq!(
        permission_calls.len(),
        1,
        "should request permission once before cancellation"
    );

    harness.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn fs_write_text_file_enforces_project_root_sandbox() {
    let agent = Arc::new(FakePermissionAgentTransport::new(
        success_initialize_response(),
    ));
    let harness = BridgeHarness::start(agent.clone()).await;

    let (mut ws, _) = harness
        .connect(ALLOWED_ORIGIN, Some(SUBPROTOCOL))
        .await
        .expect("handshake should succeed");

    // Initialize and create session
    send_initialize_request(&mut ws).await;
    let _init_response = next_message(&mut ws).await;
    send_session_new_request(&mut ws).await;
    let session_response = next_message(&mut ws).await;
    let session_payload = parse_json(&session_response);
    let session_id = session_payload
        .get("result")
        .and_then(|r| r.get("sessionId"))
        .and_then(|s| s.as_str())
        .expect("should have sessionId");

    // Test writing file outside project root - should be rejected per RAT-LWS-REQ-044
    send_json_rpc(
        &mut ws,
        json!({
            "jsonrpc": "2.0",
            "id": "write-oob-1",
            "method": "fs/write_text_file",
            "params": {
                "sessionId": session_id,
                "path": "/etc/malicious_file.txt",
                "content": "malicious content"
            }
        }),
    )
    .await;

    let message = next_message(&mut ws).await;
    let payload = parse_json(&message);

    assert_eq!(payload.get("id"), Some(&json!("write-oob-1")));

    // Should return error for out-of-bounds write (not method not found)
    let error = payload
        .get("error")
        .expect("should have error for out-of-bounds write");
    let error_code = error
        .get("code")
        .and_then(|c| c.as_i64())
        .expect("error should have numeric code");
    // Should be permission/sandbox error, not method not found (-32601)
    assert_ne!(
        error_code, -32601,
        "should be sandbox violation error, not method not found"
    );

    // Verify permission was NOT requested for out-of-bounds access
    // (sandbox check should happen before permission request)
    let permission_calls = agent.take_permission_calls().await;
    assert_eq!(
        permission_calls.len(),
        0,
        "should not request permission for out-of-bounds write"
    );

    harness.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn fs_write_text_file_permission_flow_with_allow_always() {
    let agent = Arc::new(FakePermissionAgentTransport::new(
        success_initialize_response(),
    ));
    let harness = BridgeHarness::start(agent.clone()).await;

    let (mut ws, _) = harness
        .connect(ALLOWED_ORIGIN, Some(SUBPROTOCOL))
        .await
        .expect("handshake should succeed");

    // Initialize and create session
    send_initialize_request(&mut ws).await;
    let _init_response = next_message(&mut ws).await;
    send_session_new_request(&mut ws).await;
    let session_response = next_message(&mut ws).await;
    let session_payload = parse_json(&session_response);
    let session_id = session_payload
        .get("result")
        .and_then(|r| r.get("sessionId"))
        .and_then(|s| s.as_str())
        .expect("should have sessionId");

    // Configure agent to provide allow_always permission
    agent
        .configure_permission_response(acp::RequestPermissionResponse {
            outcome: acp::RequestPermissionOutcome::Selected {
                option_id: acp::PermissionOptionId("allow_always".into()),
            },
            meta: None,
        })
        .await;

    // Test fs/write_text_file request with allow_always outcome per RAT-LWS-REQ-091
    send_json_rpc(
        &mut ws,
        json!({
            "jsonrpc": "2.0",
            "id": "write-always-1",
            "method": "fs/write_text_file",
            "params": {
                "sessionId": session_id,
                "path": "test_always.txt",
                "content": "Always allowed content"
            }
        }),
    )
    .await;

    let message = next_message(&mut ws).await;
    let payload = parse_json(&message);

    assert_eq!(payload.get("id"), Some(&json!("write-always-1")));

    // Should succeed with allow_always permission
    let result = payload
        .get("result")
        .expect("fs/write_text_file should succeed with allow_always permission");
    assert!(
        result.is_object(),
        "result should be WriteTextFileResponse object"
    );

    // Verify permission was requested and includes allow_always option
    let permission_calls = agent.take_permission_calls().await;
    assert_eq!(permission_calls.len(), 1, "should request permission once");
    let permission_request = &permission_calls[0];
    let has_allow_always = permission_request
        .options
        .iter()
        .any(|opt| opt.kind == acp::PermissionOptionKind::AllowAlways);
    assert!(
        has_allow_always,
        "should offer allow_always option per RAT-LWS-REQ-091"
    );

    harness.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn fs_write_text_file_permission_flow_with_reject_always() {
    let agent = Arc::new(FakePermissionAgentTransport::new(
        success_initialize_response(),
    ));
    let harness = BridgeHarness::start(agent.clone()).await;

    let (mut ws, _) = harness
        .connect(ALLOWED_ORIGIN, Some(SUBPROTOCOL))
        .await
        .expect("handshake should succeed");

    // Initialize and create session
    send_initialize_request(&mut ws).await;
    let _init_response = next_message(&mut ws).await;
    send_session_new_request(&mut ws).await;
    let session_response = next_message(&mut ws).await;
    let session_payload = parse_json(&session_response);
    let session_id = session_payload
        .get("result")
        .and_then(|r| r.get("sessionId"))
        .and_then(|s| s.as_str())
        .expect("should have sessionId");

    // Configure agent to provide reject_always permission
    agent
        .configure_permission_response(acp::RequestPermissionResponse {
            outcome: acp::RequestPermissionOutcome::Selected {
                option_id: acp::PermissionOptionId("reject_always".into()),
            },
            meta: None,
        })
        .await;

    // Test fs/write_text_file request with reject_always outcome per RAT-LWS-REQ-091
    send_json_rpc(
        &mut ws,
        json!({
            "jsonrpc": "2.0",
            "id": "write-reject-always-1",
            "method": "fs/write_text_file",
            "params": {
                "sessionId": session_id,
                "path": "test_reject.txt",
                "content": "Always rejected content"
            }
        }),
    )
    .await;

    let message = next_message(&mut ws).await;
    let payload = parse_json(&message);

    assert_eq!(payload.get("id"), Some(&json!("write-reject-always-1")));

    // Should return error with reject_always permission
    let error = payload
        .get("error")
        .expect("should have error when permission rejected");
    let error_code = error
        .get("code")
        .and_then(|c| c.as_i64())
        .expect("error should have numeric code");
    // Should be permission denied, not method not found
    assert_ne!(
        error_code, -32601,
        "should be permission denied error, not method not found"
    );

    // Verify permission was requested and includes reject_always option
    let permission_calls = agent.take_permission_calls().await;
    assert_eq!(permission_calls.len(), 1, "should request permission once");
    let permission_request = &permission_calls[0];
    let has_reject_always = permission_request
        .options
        .iter()
        .any(|opt| opt.kind == acp::PermissionOptionKind::RejectAlways);
    assert!(
        has_reject_always,
        "should offer reject_always option per RAT-LWS-REQ-091"
    );

    harness.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn fs_write_text_file_validates_permission_before_execution() {
    let agent = Arc::new(FakePermissionAgentTransport::new(
        success_initialize_response(),
    ));
    let harness = BridgeHarness::start(agent.clone()).await;

    let (mut ws, _) = harness
        .connect(ALLOWED_ORIGIN, Some(SUBPROTOCOL))
        .await
        .expect("handshake should succeed");

    // Initialize and create session
    send_initialize_request(&mut ws).await;
    let _init_response = next_message(&mut ws).await;
    send_session_new_request(&mut ws).await;
    let session_response = next_message(&mut ws).await;
    let session_payload = parse_json(&session_response);
    let session_id = session_payload
        .get("result")
        .and_then(|r| r.get("sessionId"))
        .and_then(|s| s.as_str())
        .expect("should have sessionId");

    // Configure agent to track execution order
    agent
        .configure_permission_response(acp::RequestPermissionResponse {
            outcome: acp::RequestPermissionOutcome::Selected {
                option_id: acp::PermissionOptionId("allow_once".into()),
            },
            meta: None,
        })
        .await;

    // Test fs/write_text_file request - should request permission BEFORE execution
    send_json_rpc(
        &mut ws,
        json!({
            "jsonrpc": "2.0",
            "id": "write-order-1",
            "method": "fs/write_text_file",
            "params": {
                "sessionId": session_id,
                "path": "test_execution_order.txt",
                "content": "Content written after permission approval"
            }
        }),
    )
    .await;

    let message = next_message(&mut ws).await;
    let payload = parse_json(&message);

    assert_eq!(payload.get("id"), Some(&json!("write-order-1")));

    // Should succeed after permission approval
    let result = payload
        .get("result")
        .expect("fs/write_text_file should succeed after permission approval");
    assert!(
        result.is_object(),
        "result should be WriteTextFileResponse object"
    );

    // Critical: Verify permission was requested before write execution per RAT-LWS-REQ-041
    let permission_calls = agent.take_permission_calls().await;
    assert_eq!(
        permission_calls.len(),
        1,
        "should request permission exactly once before write execution"
    );

    // Verify the permission request contains the correct tool call information
    let permission_request = &permission_calls[0];
    assert_eq!(permission_request.session_id.0.as_ref(), session_id);
    // The tool_call should contain information about the write operation
    // This ensures transparency about what permission is being requested

    harness.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn fs_write_text_file_caches_allow_always_permission() {
    let agent = Arc::new(FakePermissionAgentTransport::new(
        success_initialize_response(),
    ));
    let harness = BridgeHarness::start(agent.clone()).await;

    let (mut ws, _) = harness
        .connect(ALLOWED_ORIGIN, Some(SUBPROTOCOL))
        .await
        .expect("handshake should succeed");

    // Initialize and create session
    send_initialize_request(&mut ws).await;
    let _init_response = next_message(&mut ws).await;
    send_session_new_request(&mut ws).await;
    let session_response = next_message(&mut ws).await;
    let session_payload = parse_json(&session_response);
    let session_id = session_payload
        .get("result")
        .and_then(|r| r.get("sessionId"))
        .and_then(|s| s.as_str())
        .expect("should have sessionId");

    // Configure agent to provide allow_always permission on first request
    agent
        .configure_permission_response(acp::RequestPermissionResponse {
            outcome: acp::RequestPermissionOutcome::Selected {
                option_id: acp::PermissionOptionId("allow_always".into()),
            },
            meta: None,
        })
        .await;

    // First write to establish allow_always policy
    send_json_rpc(
        &mut ws,
        json!({
            "jsonrpc": "2.0",
            "id": "write-cache-1",
            "method": "fs/write_text_file",
            "params": {
                "sessionId": session_id,
                "path": "test_cache.txt",
                "content": "First write with allow_always"
            }
        }),
    )
    .await;

    let message = next_message(&mut ws).await;
    let payload = parse_json(&message);
    assert_eq!(payload.get("id"), Some(&json!("write-cache-1")));
    let _result = payload
        .get("result")
        .expect("first write should succeed with allow_always");

    // Verify permission was requested once
    let permission_calls = agent.take_permission_calls().await;
    assert_eq!(permission_calls.len(), 1, "should request permission once for first write");

    // Second write to same path - should skip permission request due to caching
    send_json_rpc(
        &mut ws,
        json!({
            "jsonrpc": "2.0",
            "id": "write-cache-2",
            "method": "fs/write_text_file",
            "params": {
                "sessionId": session_id,
                "path": "test_cache.txt",
                "content": "Second write should skip permission"
            }
        }),
    )
    .await;

    let message = next_message(&mut ws).await;
    let payload = parse_json(&message);
    assert_eq!(payload.get("id"), Some(&json!("write-cache-2")));
    let _result = payload
        .get("result")
        .expect("second write should succeed without permission request");

    // Verify NO additional permission requests were made
    let permission_calls = agent.take_permission_calls().await;
    assert_eq!(permission_calls.len(), 0, "should not request permission for cached allow_always");

    harness.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn fs_write_text_file_caches_reject_always_permission() {
    let agent = Arc::new(FakePermissionAgentTransport::new(
        success_initialize_response(),
    ));
    let harness = BridgeHarness::start(agent.clone()).await;

    let (mut ws, _) = harness
        .connect(ALLOWED_ORIGIN, Some(SUBPROTOCOL))
        .await
        .expect("handshake should succeed");

    // Initialize and create session
    send_initialize_request(&mut ws).await;
    let _init_response = next_message(&mut ws).await;
    send_session_new_request(&mut ws).await;
    let session_response = next_message(&mut ws).await;
    let session_payload = parse_json(&session_response);
    let session_id = session_payload
        .get("result")
        .and_then(|r| r.get("sessionId"))
        .and_then(|s| s.as_str())
        .expect("should have sessionId");

    // Configure agent to provide reject_always permission on first request
    agent
        .configure_permission_response(acp::RequestPermissionResponse {
            outcome: acp::RequestPermissionOutcome::Selected {
                option_id: acp::PermissionOptionId("reject_always".into()),
            },
            meta: None,
        })
        .await;

    // First write attempt - should be rejected and establish reject_always policy
    send_json_rpc(
        &mut ws,
        json!({
            "jsonrpc": "2.0",
            "id": "write-reject-cache-1",
            "method": "fs/write_text_file",
            "params": {
                "sessionId": session_id,
                "path": "test_reject_cache.txt",
                "content": "First write attempt with reject_always"
            }
        }),
    )
    .await;

    let message = next_message(&mut ws).await;
    let payload = parse_json(&message);
    assert_eq!(payload.get("id"), Some(&json!("write-reject-cache-1")));
    let _error = payload
        .get("error")
        .expect("first write should be rejected with reject_always");

    // Verify permission was requested once
    let permission_calls = agent.take_permission_calls().await;
    assert_eq!(permission_calls.len(), 1, "should request permission once for first rejection");

    // Second write attempt to same path - should fail immediately without contacting agent
    send_json_rpc(
        &mut ws,
        json!({
            "jsonrpc": "2.0",
            "id": "write-reject-cache-2",
            "method": "fs/write_text_file",
            "params": {
                "sessionId": session_id,
                "path": "test_reject_cache.txt",
                "content": "Second write should fail immediately"
            }
        }),
    )
    .await;

    let message = next_message(&mut ws).await;
    let payload = parse_json(&message);
    assert_eq!(payload.get("id"), Some(&json!("write-reject-cache-2")));
    let _error = payload
        .get("error")
        .expect("second write should fail immediately due to cached reject_always");

    // Verify NO additional permission requests were made
    let permission_calls = agent.take_permission_calls().await;
    assert_eq!(permission_calls.len(), 0, "should not request permission for cached reject_always");

    harness.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn bridge_handshake_caches_allow_always_permission_decisions() {
    let agent = Arc::new(FakePermissionAgentTransport::new(
        success_initialize_response(),
    ));
    let harness = BridgeHarness::start(agent.clone()).await;

    let (mut ws, _) = harness
        .connect(ALLOWED_ORIGIN, Some(SUBPROTOCOL))
        .await
        .expect("handshake should succeed");

    // Initialize and create session
    send_initialize_request(&mut ws).await;
    let _init_response = next_message(&mut ws).await;
    send_session_new_request(&mut ws).await;
    let session_response = next_message(&mut ws).await;
    let session_payload = parse_json(&session_response);
    let session_id = session_payload
        .get("result")
        .and_then(|r| r.get("sessionId"))
        .and_then(|s| s.as_str())
        .expect("should have sessionId");

    // Configure agent to provide allow_always permission on first request
    agent
        .configure_permission_response(acp::RequestPermissionResponse {
            outcome: acp::RequestPermissionOutcome::Selected {
                option_id: acp::PermissionOptionId("allow_always".into()),
            },
            meta: None,
        })
        .await;

    // First write to establish allow_always policy for canonical path
    send_json_rpc(
        &mut ws,
        json!({
            "jsonrpc": "2.0",
            "id": "cache-allow-1",
            "method": "fs/write_text_file",
            "params": {
                "sessionId": session_id,
                "path": "cache_allow_test.txt",
                "content": "First write establishing allow_always policy"
            }
        }),
    )
    .await;

    let message = next_message(&mut ws).await;
    let payload = parse_json(&message);
    assert_eq!(payload.get("id"), Some(&json!("cache-allow-1")));
    let _result = payload
        .get("result")
        .expect("first write should succeed with allow_always");

    // Verify permission was requested once
    let permission_calls = agent.take_permission_calls().await;
    assert_eq!(permission_calls.len(), 1, "should request permission once for first write");

    // Second write to same canonical path - should skip permission request and succeed
    send_json_rpc(
        &mut ws,
        json!({
            "jsonrpc": "2.0",
            "id": "cache-allow-2",
            "method": "fs/write_text_file",
            "params": {
                "sessionId": session_id,
                "path": "cache_allow_test.txt",
                "content": "Second write should skip permission request"
            }
        }),
    )
    .await;

    let message = next_message(&mut ws).await;
    let payload = parse_json(&message);
    assert_eq!(payload.get("id"), Some(&json!("cache-allow-2")));
    let _result = payload
        .get("result")
        .expect("second write to same canonical path should succeed without permission request");

    // Verify NO additional permission requests were made
    let permission_calls = agent.take_permission_calls().await;
    assert_eq!(permission_calls.len(), 0, "should not request permission for cached allow_always decision");

    harness.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn bridge_handshake_caches_reject_always_permission_decisions() {
    let agent = Arc::new(FakePermissionAgentTransport::new(
        success_initialize_response(),
    ));
    let harness = BridgeHarness::start(agent.clone()).await;

    let (mut ws, _) = harness
        .connect(ALLOWED_ORIGIN, Some(SUBPROTOCOL))
        .await
        .expect("handshake should succeed");

    // Initialize and create session
    send_initialize_request(&mut ws).await;
    let _init_response = next_message(&mut ws).await;
    send_session_new_request(&mut ws).await;
    let session_response = next_message(&mut ws).await;
    let session_payload = parse_json(&session_response);
    let session_id = session_payload
        .get("result")
        .and_then(|r| r.get("sessionId"))
        .and_then(|s| s.as_str())
        .expect("should have sessionId");

    // Configure agent to provide reject_always permission on first request
    agent
        .configure_permission_response(acp::RequestPermissionResponse {
            outcome: acp::RequestPermissionOutcome::Selected {
                option_id: acp::PermissionOptionId("reject_always".into()),
            },
            meta: None,
        })
        .await;

    // First write attempt - should be rejected and establish reject_always policy
    send_json_rpc(
        &mut ws,
        json!({
            "jsonrpc": "2.0",
            "id": "cache-reject-1",
            "method": "fs/write_text_file",
            "params": {
                "sessionId": session_id,
                "path": "cache_reject_test.txt",
                "content": "First write attempt establishing reject_always policy"
            }
        }),
    )
    .await;

    let message = next_message(&mut ws).await;
    let payload = parse_json(&message);
    assert_eq!(payload.get("id"), Some(&json!("cache-reject-1")));
    let _error = payload
        .get("error")
        .expect("first write should be rejected with reject_always");

    // Verify permission was requested once
    let permission_calls = agent.take_permission_calls().await;
    assert_eq!(permission_calls.len(), 1, "should request permission once for first rejection");

    // Second write attempt to same canonical path - should fail immediately without contacting agent
    send_json_rpc(
        &mut ws,
        json!({
            "jsonrpc": "2.0",
            "id": "cache-reject-2",
            "method": "fs/write_text_file",
            "params": {
                "sessionId": session_id,
                "path": "cache_reject_test.txt",
                "content": "Second write should fail immediately due to cached reject_always"
            }
        }),
    )
    .await;

    let message = next_message(&mut ws).await;
    let payload = parse_json(&message);
    assert_eq!(payload.get("id"), Some(&json!("cache-reject-2")));
    let _error = payload
        .get("error")
        .expect("second write to same canonical path should fail immediately without contacting agent");

    // Verify NO additional permission requests were made
    let permission_calls = agent.take_permission_calls().await;
    assert_eq!(permission_calls.len(), 0, "should not request permission for cached reject_always decision");

    harness.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn bridge_handshake_requests_permission_when_no_policy_exists() {
    let agent = Arc::new(FakePermissionAgentTransport::new(
        success_initialize_response(),
    ));
    let harness = BridgeHarness::start(agent.clone()).await;

    let (mut ws, _) = harness
        .connect(ALLOWED_ORIGIN, Some(SUBPROTOCOL))
        .await
        .expect("handshake should succeed");

    // Initialize and create session
    send_initialize_request(&mut ws).await;
    let _init_response = next_message(&mut ws).await;
    send_session_new_request(&mut ws).await;
    let session_response = next_message(&mut ws).await;
    let session_payload = parse_json(&session_response);
    let session_id = session_payload
        .get("result")
        .and_then(|r| r.get("sessionId"))
        .and_then(|s| s.as_str())
        .expect("should have sessionId");

    // Configure agent to provide allow_once permission
    agent
        .configure_permission_response(acp::RequestPermissionResponse {
            outcome: acp::RequestPermissionOutcome::Selected {
                option_id: acp::PermissionOptionId("allow_once".into()),
            },
            meta: None,
        })
        .await;

    // First write to a new path - should request permission since no policy exists
    send_json_rpc(
        &mut ws,
        json!({
            "jsonrpc": "2.0",
            "id": "no-policy-1",
            "method": "fs/write_text_file",
            "params": {
                "sessionId": session_id,
                "path": "no_policy_test.txt",
                "content": "First write to new path should request permission"
            }
        }),
    )
    .await;

    let message = next_message(&mut ws).await;
    let payload = parse_json(&message);
    assert_eq!(payload.get("id"), Some(&json!("no-policy-1")));
    let _result = payload
        .get("result")
        .expect("write should succeed after permission approval");

    // Verify permission was requested for the new path
    let permission_calls = agent.take_permission_calls().await;
    assert_eq!(permission_calls.len(), 1, "should request permission when no policy entry exists");

    harness.shutdown().await;
}

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
    let agent = Arc::new(FakeStreamingAgentTransport::new(success_initialize_response()));
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
    for _ in 0..10 {  // max 10 messages to avoid infinite loop
        let message = next_message(&mut ws).await;
        let payload = parse_json(&message);

        if payload.get("method").and_then(|m| m.as_str()) == Some("session/update") {
            // Verify session/update notification format per RAT-LWS-REQ-011
            assert!(payload.get("params").is_some(), "session/update must have params");
            update_count += 1;
        } else if payload.get("id") == Some(&json!("prompt-1")) {
            // This should be the final response
            let result = payload.get("result").expect("final response should have result");
            assert!(
                result.get("stopReason").is_some(),
                "final response must contain stopReason per spec"
            );
            final_response_received = true;
            break;
        }
    }

    assert!(update_count > 0, "should receive at least one session/update notification");
    assert!(final_response_received, "should receive final response with stopReason");

    harness.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn bridge_forwards_session_prompt_transparently() {
    let agent = Arc::new(FakeStreamingAgentTransport::new(success_initialize_response()));
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
    assert_eq!(prompt_calls.len(), 1, "session/prompt should be forwarded to agent");
    assert_eq!(prompt_calls[0].prompt, test_prompt);

    harness.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn bridge_session_update_preserves_json_rpc_format() {
    let agent = Arc::new(FakeStreamingAgentTransport::new(success_initialize_response()));
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
    agent.configure_streaming_updates(vec![
        json!({
            "sessionId": "test-session-id",
            "chunk": {"type": "text", "content": "Hello"},
            "index": 0
        }),
        json!({
            "sessionId": "test-session-id",
            "chunk": {"type": "text", "content": " world"},
            "index": 1
        })
    ]).await;

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
        Box::pin(async move {
            Err(AgentTransportError::NotImplemented)
        })
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
            let prompt_text = if let Some(acp::ContentBlock::Text(text_content)) = request.prompt.first() {
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
                if let Err(e) = notification_sender.send_notification("session/update", update).await {
                    eprintln!("Failed to send session/update notification: {:?}", e);
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
                    })
                ];

                for update in default_updates {
                    if let Err(e) = notification_sender.send_notification("session/update", update).await {
                        eprintln!("Failed to send default session/update notification: {:?}", e);
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

use std::future::Future;
use std::net::SocketAddr;
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
}

#[derive(Clone)]
struct FakeAgentTransport {
    state: Arc<Mutex<FakeAgentState>>,
}

impl FakeAgentTransport {
    fn new(response: acp::InitializeResponse) -> Self {
        Self {
            state: Arc::new(Mutex::new(FakeAgentState {
                initialize_calls: Vec::new(),
                initialize_response: response,
            })),
        }
    }

    async fn take_initialize_calls(&self) -> Vec<acp::InitializeRequest> {
        let mut state = self.state.lock().await;
        std::mem::take(&mut state.initialize_calls)
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
}

struct BridgeHarness {
    handle: BridgeHandle,
    addr: SocketAddr,
    _agent: Arc<FakeAgentTransport>,
}

impl BridgeHarness {
    async fn start(agent: Arc<FakeAgentTransport>) -> Self {
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

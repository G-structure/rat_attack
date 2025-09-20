//! Bridge WebSocket server implementation
//!
//! This module contains the core WebSocket server logic for the CT-BRIDGE,
//! including connection handling, client lifecycle management, and the main
//! serve function that starts the bridge server.

use std::{future::Future, pin::Pin, sync::Arc};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tokio_tungstenite::{accept_hdr_async, WebSocketStream};
use tokio_tungstenite::tungstenite::handshake::server::{Request, Response as HandshakeResponse};

use crate::bridge::{BridgeConfig, BridgeSharedConfig, BridgeHandle, BridgeError};
use crate::transport::AgentTransport;
use crate::protocol::{validate_origin, validate_subprotocol};

/// Starts a new bridge server with the given configuration and agent transport
///
/// This function creates and starts a WebSocket server that will forward ACP
/// messages between web clients and the provided agent transport.
///
/// # Arguments
/// - `config`: Bridge configuration including bind address, allowed origins, and bridge ID
/// - `transport`: Agent transport implementation that will handle ACP requests
///
/// # Returns
/// A future that resolves to a `BridgeHandle` which can be used to control the server,
/// or a `BridgeError` if the server could not be started.
///
/// # Spec Compliance
/// Implements RAT-LWS-REQ-001/002 (WebSocket upgrade and subprotocol handling)
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

        let shared = Arc::new(BridgeSharedConfig::new(BridgeConfig {
            bind_addr,
            allowed_origins,
            expected_subprotocol,
            bridge_id,
        }));

        let (shutdown_tx, shutdown_rx) = oneshot::channel();

        let join_handle =
            spawn_accept_loop(listener, shutdown_rx, shared.clone(), transport.clone());

        Ok(BridgeHandle::new(local_addr, shutdown_tx, join_handle))
    })
}

/// Spawns the main server accept loop that handles incoming connections
///
/// This function runs the main server loop that accepts new TCP connections
/// and spawns tasks to handle each WebSocket client connection.
///
/// # Arguments
/// - `listener`: TCP listener for accepting connections
/// - `shutdown_rx`: Channel receiver for shutdown signals
/// - `shared`: Shared bridge configuration
/// - `transport`: Agent transport for handling ACP requests
///
/// # Returns
/// A `JoinHandle` for the accept loop task
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

/// Errors that can occur during client handling
#[derive(Debug)]
enum ClientError {
    /// Error during WebSocket handshake
    Handshake(tokio_tungstenite::tungstenite::Error),
    /// Error during WebSocket communication
    WebSocket(tokio_tungstenite::tungstenite::Error),
}

/// Handles a single client connection from TCP stream to WebSocket closure
///
/// This function manages the complete lifecycle of a client connection,
/// from WebSocket upgrade through message processing until disconnection.
///
/// # Arguments
/// - `stream`: The TCP stream for the client connection
/// - `shared`: Shared bridge configuration and state
/// - `transport`: Agent transport for handling ACP requests
///
/// # Returns
/// - `Ok(())` if the client disconnected cleanly
/// - `Err(ClientError)` if there was an error during handling
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

/// Performs WebSocket upgrade with validation
///
/// This function handles the WebSocket handshake, validating the origin
/// and subprotocol according to RAT-LWS-REQ-001 and RAT-LWS-REQ-002.
///
/// # Arguments
/// - `stream`: The TCP stream to upgrade
/// - `shared`: Shared configuration containing allowed origins and subprotocol
///
/// # Returns
/// - `Ok(WebSocketStream)` if the upgrade was successful
/// - `Err(tungstenite::Error)` if the upgrade failed validation
async fn accept_client(
    stream: TcpStream,
    shared: Arc<BridgeSharedConfig>,
) -> Result<WebSocketStream<TcpStream>, tokio_tungstenite::tungstenite::Error> {
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

/// Handles WebSocket message processing for a connected client
///
/// This function processes incoming WebSocket messages and forwards them
/// to the protocol request handler. It manages the WebSocket message loop
/// until the client disconnects.
///
/// # Arguments
/// - `stream`: The WebSocket stream for communication
/// - `shared`: Shared bridge configuration and state
/// - `transport`: Agent transport for handling ACP requests
///
/// # Returns
/// - `Ok(())` if the client disconnected cleanly
/// - `Err(tungstenite::Error)` if there was a WebSocket error
async fn handle_websocket(
    stream: WebSocketStream<TcpStream>,
    shared: Arc<BridgeSharedConfig>,
    transport: Arc<dyn AgentTransport>,
) -> Result<(), tokio_tungstenite::tungstenite::Error> {
    // Import the request processor from protocol module
    use crate::protocol::handle_websocket_messages;

    handle_websocket_messages(stream, shared, transport).await
}
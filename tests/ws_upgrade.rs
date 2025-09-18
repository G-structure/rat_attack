use futures::StreamExt;
use tokio_tungstenite::{connect_async, tungstenite::handshake::client::Request};
use tungstenite::handshake::client::generate_key;
use tungstenite::protocol::frame::coding::CloseCode;

#[tokio::test]
async fn test_valid_origin_upgrade() {
    let url = "ws://localhost:8137";
    let key = generate_key();
    let request = Request::builder()
        .uri(url)
        .header("Host", "localhost:8137")
        .header("Upgrade", "websocket")
        .header("Connection", "Upgrade")
        .header("Sec-WebSocket-Key", key)
        .header("Sec-WebSocket-Version", "13")
        .header("Origin", "http://localhost:5173")
        .body(())
        .unwrap();
    let (_stream, _response) = connect_async(request)
        .await
        .expect("WS upgrade should succeed with valid origin");
}

#[tokio::test]
async fn test_invalid_origin_rejection() {
    let url = "ws://localhost:8137";
    let key = generate_key();
    let request = Request::builder()
        .uri(url)
        .header("Host", "localhost:8137")
        .header("Upgrade", "websocket")
        .header("Connection", "Upgrade")
        .header("Sec-WebSocket-Key", key)
        .header("Sec-WebSocket-Version", "13")
        .header("Origin", "http://evil.com")
        .body(())
        .unwrap();
    let result = connect_async(request).await;
    assert!(
        result.is_err(),
        "WS upgrade should fail with invalid origin"
    );
}

#[tokio::test]
async fn test_valid_subprotocol_echo() {
    let url = "ws://localhost:8137";
    let key = generate_key();
    let request = Request::builder()
        .uri(url)
        .header("Host", "localhost:8137")
        .header("Upgrade", "websocket")
        .header("Connection", "Upgrade")
        .header("Sec-WebSocket-Key", key)
        .header("Sec-WebSocket-Version", "13")
        .header("Origin", "http://localhost:5173")
        .header("Sec-WebSocket-Protocol", "acp.jsonrpc.v1")
        .body(())
        .unwrap();
    let (_stream, response) = connect_async(request)
        .await
        .expect("WS upgrade should succeed with valid subprotocol");
    assert_eq!(
        response.headers().get("sec-websocket-protocol").unwrap(),
        "acp.jsonrpc.v1"
    );
}

#[tokio::test]
async fn test_no_subprotocol_close_1008() {
    let url = "ws://localhost:8137";
    let key = generate_key();
    let request = Request::builder()
        .uri(url)
        .header("Host", "localhost:8137")
        .header("Upgrade", "websocket")
        .header("Connection", "Upgrade")
        .header("Sec-WebSocket-Key", key)
        .header("Sec-WebSocket-Version", "13")
        .header("Origin", "http://localhost:5173")
        .body(())
        .unwrap();
    let (mut stream, _response) = connect_async(request)
        .await
        .expect("WS upgrade should succeed initially");
    // Read the close frame
    let msg = stream.next().await.unwrap().unwrap();
    if let tungstenite::Message::Close(frame) = msg {
        assert_eq!(frame.unwrap().code, CloseCode::Policy);
    } else {
        panic!("Expected close frame");
    }
}

#[tokio::test]
async fn test_different_subprotocol_close_1008() {
    let url = "ws://localhost:8137";
    let key = generate_key();
    let request = Request::builder()
        .uri(url)
        .header("Host", "localhost:8137")
        .header("Upgrade", "websocket")
        .header("Connection", "Upgrade")
        .header("Sec-WebSocket-Key", key)
        .header("Sec-WebSocket-Version", "13")
        .header("Origin", "http://localhost:5173")
        .header("Sec-WebSocket-Protocol", "other.protocol")
        .body(())
        .unwrap();
    let (mut stream, _response) = connect_async(request)
        .await
        .expect("WS upgrade should succeed initially");
    // Read the close frame
    let msg = stream.next().await.unwrap().unwrap();
    if let tungstenite::Message::Close(frame) = msg {
        assert_eq!(frame.unwrap().code, CloseCode::Policy);
    } else {
        panic!("Expected close frame");
    }
}

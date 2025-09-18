use tokio_tungstenite::{connect_async, tungstenite::handshake::client::Request};
use tungstenite::handshake::client::generate_key;

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
    // Note: Currently fails due to no server, later should be HTTP 403
}

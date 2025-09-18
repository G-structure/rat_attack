use futures::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio_tungstenite::{connect_async, tungstenite::handshake::client::Request};
use tungstenite::handshake::client::generate_key;
use tungstenite::protocol::frame::coding::CloseCode;
use tungstenite::Message;

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

#[tokio::test]
async fn test_multiple_subprotocols_with_acp_accept() {
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
        .header("Sec-WebSocket-Protocol", "acp.jsonrpc.v1, other.protocol")
        .body(())
        .unwrap();
    let (_stream, response) = connect_async(request)
        .await
        .expect("WS upgrade should succeed with acp in multiple subprotocols");
    assert_eq!(
        response.headers().get("sec-websocket-protocol").unwrap(),
        "acp.jsonrpc.v1"
    );
}

#[tokio::test]
async fn test_multiple_subprotocols_without_acp_reject() {
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
        .header("Sec-WebSocket-Protocol", "other.protocol, another.one")
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
async fn test_initialize_bridge_id_response() {
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
    let (mut stream, _response) = connect_async(request)
        .await
        .expect("WS upgrade should succeed with ACP subprotocol");

    // First initialize request
    let init_request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {}
    });
    stream
        .send(Message::Text(init_request.to_string()))
        .await
        .unwrap();

    // Receive response
    let msg = stream.next().await.unwrap().unwrap();
    let response_text = match msg {
        Message::Text(text) => text,
        _ => panic!("Expected text message"),
    };
    let response: Value = serde_json::from_str(&response_text).unwrap();

    // Assert response structure
    assert_eq!(response["jsonrpc"], "2.0");
    assert_eq!(response["id"], 1);
    assert!(response["result"].is_object());
    let result = &response["result"];
    assert!(result["_meta"].is_object());
    let meta = &result["_meta"];
    assert!(meta["bridgeId"].is_string());
    let bridge_id = meta["bridgeId"].as_str().unwrap();
    assert!(!bridge_id.is_empty());

    // Second initialize request
    let init_request2 = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "initialize",
        "params": {}
    });
    stream
        .send(Message::Text(init_request2.to_string()))
        .await
        .unwrap();

    // Receive second response
    let msg2 = stream.next().await.unwrap().unwrap();
    let response_text2 = match msg2 {
        Message::Text(text) => text,
        _ => panic!("Expected text message"),
    };
    let response2: Value = serde_json::from_str(&response_text2).unwrap();

    // Assert second response
    assert_eq!(response2["jsonrpc"], "2.0");
    assert_eq!(response2["id"], 2);
    assert!(response2["result"].is_object());
    let result2 = &response2["result"];
    assert!(result2["_meta"].is_object());
    let meta2 = &result2["_meta"];
    assert!(meta2["bridgeId"].is_string());
    let bridge_id2 = meta2["bridgeId"].as_str().unwrap();
    assert!(!bridge_id2.is_empty());

    // Assert bridgeId is the same
    assert_eq!(bridge_id, bridge_id2);
}

use futures::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;
use tokio_tungstenite::accept_hdr_async;
use tungstenite::handshake::server::{Request, Response};
use tungstenite::http;
use tungstenite::{protocol::frame::CloseFrame, Message};
use uuid::Uuid;

struct Config {
    bind_addr: String,
    allowed_origins: Vec<String>,
    bridge_id: String,
    project_roots: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            bind_addr: "127.0.0.1:8137".to_string(),
            allowed_origins: vec!["http://localhost:5173".to_string()],
            bridge_id: Uuid::new_v4().to_string(),
            project_roots: vec![std::env::current_dir()
                .unwrap()
                .to_string_lossy()
                .to_string()],
        }
    }
}

fn has_valid_fs_capabilities(params: Option<&Value>) -> bool {
    let capabilities = params.and_then(|p| p.get("capabilities"));
    let fs_caps = capabilities.and_then(|c| c.get("fs"));
    let read_cap = fs_caps
        .and_then(|fs| fs.get("readTextFile"))
        .and_then(|v| v.as_bool());
    let write_cap = fs_caps
        .and_then(|fs| fs.get("writeTextFile"))
        .and_then(|v| v.as_bool());

    read_cap == Some(true) && write_cap == Some(true)
}

fn is_path_within_project_roots(path: &str, project_roots: &[String]) -> bool {
    use std::path::Path;
    let requested_path = Path::new(path);
    for root in project_roots {
        let root_path = Path::new(root);
        if requested_path.starts_with(root_path) {
            return true;
        }
    }
    false
}

fn handle_jsonrpc_request(
    request: &Value,
    bridge_id: &str,
    project_roots: &[String],
) -> Option<Value> {
    let method = request.get("method")?.as_str()?;
    let id = request.get("id")?;

    match method {
        "initialize" => {
            let params = request.get("params");

            if has_valid_fs_capabilities(params) {
                let response = json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "_meta": {
                            "bridgeId": bridge_id
                        },
                        "capabilities": {
                            "fs": {
                                "readTextFile": true,
                                "writeTextFile": true
                            }
                        }
                    }
                });
                Some(response)
            } else {
                let error_response = json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "error": {
                        "code": -32602,
                        "message": "Missing required fs capabilities: readTextFile and writeTextFile must both be true"
                    }
                });
                Some(error_response)
            }
        }
        "fs/read_text_file" => {
            let params = request.get("params")?;
            let path = params.get("path")?.as_str()?;

            if is_path_within_project_roots(path, project_roots) {
                match std::fs::read_to_string(path) {
                    Ok(content) => {
                        let response = json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "result": {
                                "content": content
                            }
                        });
                        Some(response)
                    }
                    Err(e) => {
                        let error_response = json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "error": {
                                "code": -32000,
                                "message": format!("Failed to read file: {}", e),
                                "data": {
                                    "details": format!("Could not read file '{}': {}", path, e)
                                }
                            }
                        });
                        Some(error_response)
                    }
                }
            } else {
                let error_response = json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "error": {
                        "code": -32000,
                        "message": "Access denied: path is outside configured project roots",
                        "data": {
                            "details": format!("The requested path '{}' is not within any configured project root", path)
                        }
                    }
                });
                Some(error_response)
            }
        }
        _ => None, // Unknown method, no response
    }
}

async fn handle_connection(
    mut ws_stream: tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>,
    bridge_id: String,
    project_roots: Vec<String>,
) {
    while let Some(msg) = ws_stream.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                if let Ok(request) = serde_json::from_str::<Value>(&text) {
                    if let Some(response) =
                        handle_jsonrpc_request(&request, &bridge_id, &project_roots)
                    {
                        if let Err(e) = ws_stream.send(Message::Text(response.to_string())).await {
                            eprintln!("Failed to send response: {e:?}");
                            break;
                        }
                    }
                } else {
                    eprintln!("Failed to parse JSON-RPC request: {text}");
                }
            }
            Ok(Message::Close(_)) => break,
            Ok(_) => {} // Ignore other message types
            Err(e) => {
                eprintln!("WebSocket error: {e:?}");
                break;
            }
        }
    }
}

async fn run_server(config: Config) -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind(&config.bind_addr).await?;
    println!("Listening on: {}", config.bind_addr);
    let bridge_id = config.bridge_id.clone();
    let project_roots = config.project_roots.clone();

    loop {
        let (stream, _) = listener.accept().await?;
        let allowed_origins = config.allowed_origins.clone();
        let bridge_id = bridge_id.clone();
        let project_roots = project_roots.clone();
        tokio::spawn(async move {
            let valid_subproto = Arc::new(Mutex::new(false));
            let callback_valid = Arc::clone(&valid_subproto);
            let callback = move |req: &Request, mut res: Response| {
                if let Some(origin) = req.headers().get("origin") {
                    let origin_str = origin.to_str().unwrap_or("");
                    if allowed_origins.contains(&String::from(origin_str)) {
                        let offered_proto = req
                            .headers()
                            .get("sec-websocket-protocol")
                            .and_then(|v| v.to_str().ok());
                        let is_valid = offered_proto
                            .map(|s| {
                                s.split(',')
                                    .map(|p| p.trim())
                                    .any(|p| p == "acp.jsonrpc.v1")
                            })
                            .unwrap_or(false);
                        *callback_valid.lock().unwrap() = is_valid;
                        if is_valid {
                            res.headers_mut().insert(
                                "sec-websocket-protocol",
                                "acp.jsonrpc.v1".parse().unwrap(),
                            );
                        }
                        Ok(res)
                    } else {
                        let forbidden = http::Response::builder().status(403).body(None).unwrap();
                        Err(forbidden)
                    }
                } else {
                    let forbidden = http::Response::builder().status(403).body(None).unwrap();
                    Err(forbidden)
                }
            };
            match accept_hdr_async(stream, callback).await {
                Ok(ws_stream) => {
                    if !*valid_subproto.lock().unwrap() {
                        let close_frame = CloseFrame {
                            code: tungstenite::protocol::frame::coding::CloseCode::Policy,
                            reason: "Invalid or missing subprotocol".into(),
                        };
                        let mut ws_stream = ws_stream;
                        let _ = ws_stream.send(Message::Close(Some(close_frame))).await;
                    } else {
                        handle_connection(ws_stream, bridge_id, project_roots).await;
                    }
                }
                Err(e) => {
                    eprintln!("WebSocket upgrade failed: {e:?}");
                }
            }
        });
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("CT-BRIDGE starting...");
    let config = Config::default();
    run_server(config).await
}

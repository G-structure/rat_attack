use futures::SinkExt;
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;
use tokio_tungstenite::accept_hdr_async;
use tungstenite::handshake::server::{Request, Response};
use tungstenite::http;
use tungstenite::{protocol::frame::CloseFrame, Message};

struct Config {
    bind_addr: String,
    allowed_origins: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            bind_addr: "127.0.0.1:8137".to_string(),
            allowed_origins: vec!["http://localhost:5173".to_string()],
        }
    }
}

async fn run_server(config: Config) -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind(&config.bind_addr).await?;
    println!("Listening on: {}", config.bind_addr);

    loop {
        let (stream, _) = listener.accept().await?;
        let allowed_origins = config.allowed_origins.clone();
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
                Ok(mut ws_stream) => {
                    if !*valid_subproto.lock().unwrap() {
                        let close_frame = CloseFrame {
                            code: tungstenite::protocol::frame::coding::CloseCode::Policy,
                            reason: "Invalid or missing subprotocol".into(),
                        };
                        let _ = ws_stream.send(Message::Close(Some(close_frame))).await;
                    } else {
                        // For now, just drop
                        drop(ws_stream);
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

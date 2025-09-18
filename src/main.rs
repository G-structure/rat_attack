use tokio::net::TcpListener;
use tokio_tungstenite::accept_hdr_async;
use tungstenite::handshake::server::{Request, Response};
use tungstenite::http;

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
            let callback = |req: &Request, res: Response| {
                if let Some(origin) = req.headers().get("origin") {
                    let origin_str = origin.to_str().unwrap_or("");
                    if allowed_origins.contains(&String::from(origin_str)) {
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
                    // For now, just accept and drop
                    drop(ws_stream);
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

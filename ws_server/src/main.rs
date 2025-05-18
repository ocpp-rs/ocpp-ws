use log::{error, info};
use std::net::SocketAddr;
use std::path::PathBuf;
use ws_rs::server::{ClientId, ServerHandler, WsMessage, WsServer, WsServerConfig};

#[derive(Default)]
struct MyWsHandler;

impl ServerHandler for MyWsHandler {
    fn on_connect(&self, client_id: ClientId, addr: SocketAddr) {
        info!("Client connected: {} from {}", client_id.0, addr);
    }

    fn on_disconnect(&self, client_id: ClientId) {
        info!("Client disconnected: {}", client_id.0);
    }

    fn on_message(&self, client_id: ClientId, message: WsMessage) -> Option<WsMessage> {
        match &message {
            WsMessage::Text(text) => {
                info!("Text message from {}: {}", client_id.0, text);

                // Echo back the message with a prefix
                Some(WsMessage::Text(format!("Server received: {}", text)))
            }
            WsMessage::Binary(data) => {
                info!("Binary message from {}: {} bytes", client_id.0, data.len());

                // Echo back the binary data
                Some(WsMessage::Binary(data.clone()))
            }
            WsMessage::Close => {
                info!("Close request from {}", client_id.0);
                Some(WsMessage::Close)
            }
        }
    }

    fn on_error(&self, client_id: Option<ClientId>, error: String) {
        match client_id {
            Some(id) => error!("Error for client {}: {}", id.0, error),
            None => error!("Server error: {}", error),
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logger
    env_logger::init();

    // Define server configuration
    let config = WsServerConfig {
        addr: "127.0.0.1:9999".to_string(), // Using port 9001 instead of 9000
        cert_path: PathBuf::from("./crate_cert/a_cert.pem"),
        key_path: PathBuf::from("./crate_cert/a_key.pem"),
        ca_cert_path: PathBuf::from("./crate_cert/ca_cert.pem"),
        max_connections: 100,
        connection_timeout: 30,
        client_cert_required: true,
    };

    // Create handler
    let handler = MyWsHandler::default();

    // Create and start server
    info!("Starting WebSocket server...");
    let server =
        WsServer::new(config, handler).map_err(|e| format!("Failed to create server: {}", e))?;

    // Start the server (this will block until the server stops)
    server
        .start()
        .await
        .map_err(|e| format!("Server error: {}", e))?;

    Ok(())
}

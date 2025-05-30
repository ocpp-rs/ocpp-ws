use log::{error, info};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use ws_rs::server::{ClientId, ServerHandler, WsMessage, WsServer, WsServerConfig};
use ws_rs::path_utils::{PathInfo, parse_ocpp_station_id, parse_api_path};

/// Simple WebSocket server handler
#[derive(Clone)]
struct SimpleHandler;

impl SimpleHandler {
    fn new() -> Self {
        Self
    }
}

impl ServerHandler for SimpleHandler {
    fn on_connect(&self, client_id: ClientId, addr: SocketAddr, path: String) {
        info!("Client connected: {} from {} with path: {}", client_id.0, addr, path);
    }

    fn on_connect_with_path(&self, client_id: ClientId, addr: SocketAddr, path_info: PathInfo) {
        info!("Client connected: {} from {}", client_id.0, addr);
        info!("  Raw path: {}", path_info.raw());
        info!("  Decoded path: {}", path_info.decoded());
        info!("  URL suffix: {}", path_info.suffix());
        info!("  Decoded suffix: {}", path_info.decoded_suffix());

        // Check if this is a root path connection
        if path_info.is_root() {
            info!("  → Root path connection");
            return;
        }

        // Parse OCPP station ID if applicable
        if let Some(station_id) = parse_ocpp_station_id(&path_info) {
            info!("  → OCPP Charging Station ID: {}", station_id);
            if station_id.contains('|') {
                info!("    Station ID contains special characters (properly decoded)");
            }
        }
        // Parse API path if applicable
        else if let Some((version, endpoint)) = parse_api_path(&path_info) {
            info!("  → API Connection - Version: {}, Endpoint: {}", version, endpoint);
        }
        // Handle other path types
        else {
            let segments = path_info.segments();
            if !segments.is_empty() {
                info!("  → Custom path - First segment: {}, Total segments: {}",
                      segments[0], segments.len());
            }
        }
    }

    fn on_disconnect(&self, client_id: ClientId) {
        info!("Client disconnected: {}", client_id.0);
    }

    fn on_message(&self, client_id: ClientId, message: WsMessage) -> Option<WsMessage> {
        match &message {
            WsMessage::Text(text) => {
                info!("Text message from {}: {}", client_id.0, text);

                // Simple echo with prefix
                Some(WsMessage::Text(format!("Echo: {}", text)))
            }
            WsMessage::Binary(data) => {
                info!("Binary message from {}: {} bytes", client_id.0, data.len());
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
    let handler = SimpleHandler::new();

    // Create server
    info!("Starting WebSocket server...");
    let server = Arc::new(
        WsServer::new(config, handler).map_err(|e| format!("Failed to create server: {}", e))?
    );

    info!("Server ready with URL suffix extraction and client handle functionality...");

    // Spawn a periodic task to monitor clients
    let server_clone = server.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(30));

        loop {
            interval.tick().await;

            let clients = server_clone.client_list().await;
            if !clients.is_empty() {
                info!("Connected clients: {}", clients.len());

                // Show URL suffixes for all clients
                let all_suffixes = server_clone.get_all_client_url_suffixes_decoded().await;
                for (client_id, suffix) in all_suffixes {
                    info!("  {} -> '{}'", client_id.0, suffix);
                }
            }
        }
    });

    // Start the server (this will block until the server stops)
    server
        .start()
        .await
        .map_err(|e| format!("Server error: {}", e))?;

    Ok(())
}

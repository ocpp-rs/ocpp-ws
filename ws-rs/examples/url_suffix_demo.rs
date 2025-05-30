use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use log::{info, error};
use tokio::time::sleep;
use ws_rs::server::{WsServer, WsServerConfig, ServerHandler, ClientId, WsMessage};
use ws_rs::path_utils::{PathInfo, parse_ocpp_station_id, parse_api_path};

/// Example handler that demonstrates URL suffix extraction
#[derive(Clone)]
struct UrlSuffixDemo {
    server: Option<Arc<WsServer>>,
}

impl UrlSuffixDemo {
    fn new() -> Self {
        Self { server: None }
    }
    
    fn set_server(&mut self, server: Arc<WsServer>) {
        self.server = Some(server);
    }
}

impl ServerHandler for UrlSuffixDemo {
    fn on_connect(&self, client_id: ClientId, addr: SocketAddr, path: String) {
        info!("Client connected: {} from {} with path: {}", client_id.0, addr, path);
    }

    fn on_connect_with_path(&self, client_id: ClientId, addr: SocketAddr, path_info: PathInfo) {
        info!("=== New Client Connection ===");
        info!("Client ID: {}", client_id.0);
        info!("Address: {}", addr);
        info!("Raw path: '{}'", path_info.raw());
        info!("Decoded path: '{}'", path_info.decoded());
        info!("URL suffix: '{}'", path_info.suffix());
        info!("Decoded URL suffix: '{}'", path_info.decoded_suffix());
        
        // Analyze the path
        if path_info.is_root() {
            info!("→ Root path connection");
        } else {
            let segments = path_info.segments();
            info!("→ Path segments: {:?}", segments);
            
            if let Some(first) = path_info.first_segment() {
                info!("→ First segment: '{}'", first);
            }
            
            if let Some(last) = path_info.last_segment() {
                info!("→ Last segment: '{}'", last);
            }
            
            // Check for OCPP station ID
            if let Some(station_id) = parse_ocpp_station_id(&path_info) {
                info!("→ OCPP Charging Station ID: '{}'", station_id);
            }
            
            // Check for API path
            if let Some((version, endpoint)) = parse_api_path(&path_info) {
                info!("→ API Connection - Version: '{}', Endpoint: '{}'", version, endpoint);
            }
        }
        
        // Demonstrate server URL suffix extraction
        if let Some(server) = &self.server {
            let server_clone = server.clone();
            let client_id_clone = client_id.clone();
            
            tokio::spawn(async move {
                sleep(Duration::from_millis(100)).await;
                
                // Test the new URL suffix methods
                if let Some(suffix) = server_clone.get_client_url_suffix(&client_id_clone).await {
                    info!("Server extracted URL suffix: '{}'", suffix);
                }
                
                if let Some(decoded_suffix) = server_clone.get_client_url_suffix_decoded(&client_id_clone).await {
                    info!("Server extracted decoded URL suffix: '{}'", decoded_suffix);
                }
                
                if let Some(path_info) = server_clone.get_client_path_info(&client_id_clone).await {
                    info!("Server extracted path info:");
                    info!("  Raw: '{}'", path_info.raw());
                    info!("  Decoded: '{}'", path_info.decoded());
                    info!("  Suffix: '{}'", path_info.suffix());
                    info!("  Decoded suffix: '{}'", path_info.decoded_suffix());
                }
            });
        }
        
        info!("=== End Connection Info ===");
    }

    fn on_disconnect(&self, client_id: ClientId) {
        info!("Client disconnected: {}", client_id.0);
    }

    fn on_message(&self, client_id: ClientId, message: WsMessage) -> Option<WsMessage> {
        match &message {
            WsMessage::Text(text) => {
                info!("Text message from {}: {}", client_id.0, text);
                
                // Special commands to demonstrate URL suffix functionality
                if text == "get_suffix" {
                    if let Some(server) = &self.server {
                        let server_clone = server.clone();
                        let client_id_clone = client_id.clone();
                        
                        tokio::spawn(async move {
                            if let Some(handle) = server_clone.get_client_handle(&client_id_clone).await {
                                if let Some(suffix) = server_clone.get_client_url_suffix(&client_id_clone).await {
                                    let response = format!("Your URL suffix: '{}'", suffix);
                                    let _ = handle.send_message(WsMessage::Text(response)).await;
                                }
                            }
                        });
                    }
                    return Some(WsMessage::Text("Getting your URL suffix...".to_string()));
                }
                
                if text == "get_decoded_suffix" {
                    if let Some(server) = &self.server {
                        let server_clone = server.clone();
                        let client_id_clone = client_id.clone();
                        
                        tokio::spawn(async move {
                            if let Some(handle) = server_clone.get_client_handle(&client_id_clone).await {
                                if let Some(decoded_suffix) = server_clone.get_client_url_suffix_decoded(&client_id_clone).await {
                                    let response = format!("Your decoded URL suffix: '{}'", decoded_suffix);
                                    let _ = handle.send_message(WsMessage::Text(response)).await;
                                }
                            }
                        });
                    }
                    return Some(WsMessage::Text("Getting your decoded URL suffix...".to_string()));
                }
                
                if text == "list_all_suffixes" {
                    if let Some(server) = &self.server {
                        let server_clone = server.clone();
                        let client_id_clone = client_id.clone();
                        
                        tokio::spawn(async move {
                            if let Some(handle) = server_clone.get_client_handle(&client_id_clone).await {
                                let all_suffixes = server_clone.get_all_client_url_suffixes().await;
                                let mut response = "All client URL suffixes:\n".to_string();
                                for (id, suffix) in all_suffixes {
                                    response.push_str(&format!("  {} -> '{}'\n", id.0, suffix));
                                }
                                let _ = handle.send_message(WsMessage::Text(response)).await;
                            }
                        });
                    }
                    return Some(WsMessage::Text("Listing all client URL suffixes...".to_string()));
                }
                
                // Echo back the message
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
        addr: "127.0.0.1:9999".to_string(),
        cert_path: PathBuf::from("./crate_cert/a_cert.pem"),
        key_path: PathBuf::from("./crate_cert/a_key.pem"),
        ca_cert_path: PathBuf::from("./crate_cert/ca_cert.pem"),
        max_connections: 100,
        connection_timeout: 30,
        client_cert_required: true,
    };

    // Create handler
    let mut handler = UrlSuffixDemo::new();

    // Create server
    info!("Starting WebSocket server with URL suffix extraction demo...");
    let server = Arc::new(
        WsServer::new(config, handler.clone()).map_err(|e| format!("Failed to create server: {}", e))?
    );

    // Set server reference in handler
    handler.set_server(server.clone());

    info!("=== URL Suffix Demo Server ===");
    info!("Connect with different URL paths to test:");
    info!("  wss://127.0.0.1:9999/ocpp/CS001");
    info!("  wss://127.0.0.1:9999/ocpp/RDAM%7C123");
    info!("  wss://127.0.0.1:9999/api/v1/websocket");
    info!("  wss://127.0.0.1:9999/custom/path/here");
    info!("");
    info!("Send these messages to test URL suffix extraction:");
    info!("  'get_suffix' - Get your URL suffix");
    info!("  'get_decoded_suffix' - Get your decoded URL suffix");
    info!("  'list_all_suffixes' - List all client URL suffixes");
    info!("================================");

    // Demonstrate periodic URL suffix monitoring
    let server_clone = server.clone();
    tokio::spawn(async move {
        loop {
            sleep(Duration::from_secs(15)).await;
            
            let all_suffixes = server_clone.get_all_client_url_suffixes().await;
            let all_decoded = server_clone.get_all_client_url_suffixes_decoded().await;
            
            if !all_suffixes.is_empty() {
                info!("=== Periodic URL Suffix Report ===");
                info!("Raw URL suffixes:");
                for (client_id, suffix) in &all_suffixes {
                    info!("  {} -> '{}'", client_id.0, suffix);
                }
                
                info!("Decoded URL suffixes:");
                for (client_id, decoded) in &all_decoded {
                    info!("  {} -> '{}'", client_id.0, decoded);
                }
                info!("=== End Report ===");
            }
        }
    });

    // Start the server
    server.start().await.map_err(|e| format!("Server error: {}", e))?;

    Ok(())
}

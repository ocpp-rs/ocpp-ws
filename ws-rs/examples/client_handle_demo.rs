use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use log::{info, error};
use tokio::time::sleep;
use ws_rs::server::{WsServer, WsServerConfig, ServerHandler, ClientId, WsMessage};
use ws_rs::path_utils::PathInfo;

/// Example handler that demonstrates client handle usage
#[derive(Clone)]
struct ClientHandleDemo {
    server: Option<Arc<WsServer>>,
}

impl ClientHandleDemo {
    fn new() -> Self {
        Self { server: None }
    }
    
    fn set_server(&mut self, server: Arc<WsServer>) {
        self.server = Some(server);
    }
}

impl ServerHandler for ClientHandleDemo {
    fn on_connect(&self, client_id: ClientId, addr: SocketAddr, path: String) {
        info!("Client connected: {} from {} with path: {}", client_id.0, addr, path);
    }

    fn on_connect_with_path(&self, client_id: ClientId, addr: SocketAddr, path_info: PathInfo) {
        info!("Client connected: {} from {}", client_id.0, addr);
        info!("  Raw path: {}", path_info.raw());
        info!("  Decoded path: {}", path_info.decoded());
        info!("  URL suffix: {}", path_info.suffix());
        
        // Demonstrate using client handles
        if let Some(server) = &self.server {
            let server_clone = server.clone();
            let client_id_clone = client_id.clone();
            
            tokio::spawn(async move {
                // Wait a moment for the connection to be fully established
                sleep(Duration::from_millis(100)).await;
                
                // Get a handle for this client
                if let Some(handle) = server_clone.get_client_handle(&client_id_clone).await {
                    info!("Got handle for client: {}", handle.client_id().0);
                    
                    // Send a welcome message
                    if let Err(e) = handle.send_message(WsMessage::Text("Welcome! You are connected.".to_string())).await {
                        error!("Failed to send welcome message: {}", e);
                        return;
                    }

                    info!("Successfully initialized client handle for {}", client_id_clone.0);

                    // Demonstrate reading messages from the client
                    tokio::spawn(async move {
                        info!("Starting message reader for client {}", client_id_clone.0);
                        while let Some(message) = handle.read_message().await {
                            match message {
                                WsMessage::Text(text) => {
                                    info!("Handle received text from {}: {}", client_id_clone.0, text);
                                    // Echo back with a prefix
                                    let response = format!("Echo from handle: {}", text);
                                    let _ = handle.send_message(WsMessage::Text(response)).await;
                                }
                                WsMessage::Binary(data) => {
                                    info!("Handle received binary from {}: {} bytes", client_id_clone.0, data.len());
                                    // Echo back the binary data
                                    let _ = handle.send_message(WsMessage::Binary(data)).await;
                                }
                                WsMessage::Close => {
                                    info!("Handle received close from {}", client_id_clone.0);
                                    break;
                                }
                            }
                        }
                        info!("Message reader for client {} ended", client_id_clone.0);
                    });
                } else {
                    error!("Failed to get handle for client: {}", client_id_clone.0);
                }
            });
        }
    }

    fn on_disconnect(&self, client_id: ClientId) {
        info!("Client disconnected: {}", client_id.0);
    }

    fn on_message(&self, client_id: ClientId, message: WsMessage) -> Option<WsMessage> {
        match &message {
            WsMessage::Text(text) => {
                info!("Text message from {}: {}", client_id.0, text);
                
                // Demonstrate handle usage in message processing
                if let Some(server) = &self.server {
                    let server_clone = server.clone();
                    let client_id_clone = client_id.clone();
                    let text_clone = text.clone();

                    tokio::spawn(async move {
                        if let Some(handle) = server_clone.get_client_handle(&client_id_clone).await {
                            // Handle different commands
                            match text_clone.as_str() {
                                "ping" => {
                                    // Send a pong response
                                    let _ = handle.send_message(WsMessage::Text("pong".to_string())).await;
                                }
                                "status" => {
                                    // Check connection status
                                    let connected = handle.is_connected().await;
                                    let response = format!("Connected: {}", connected);
                                    let _ = handle.send_message(WsMessage::Text(response)).await;
                                }
                                "info" => {
                                    // Send client information
                                    let response = format!("Client ID: {}, Address: {}, Path: {}",
                                        handle.client_id().0, handle.addr(), handle.path_info().decoded());
                                    let _ = handle.send_message(WsMessage::Text(response)).await;
                                }
                                _ => {
                                    // Echo back with handle info
                                    let response = format!("Handle echo: {}", text_clone);
                                    let _ = handle.send_message(WsMessage::Text(response)).await;
                                }
                            }
                        }
                    });
                }
                
                // Return immediate response
                Some(WsMessage::Text(format!("Received: {}", text)))
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
    let mut handler = ClientHandleDemo::new();

    // Create server
    info!("Starting WebSocket server with client handle demo...");
    let server = Arc::new(
        WsServer::new(config, handler.clone()).map_err(|e| format!("Failed to create server: {}", e))?
    );

    // Set server reference in handler
    handler.set_server(server.clone());

    // Demonstrate handle management
    let server_clone = server.clone();
    tokio::spawn(async move {
        loop {
            sleep(Duration::from_secs(10)).await;
            
            let handle_count = server_clone.client_handle_count().await;
            let client_count = server_clone.client_count().await;
            
            info!("Status: {} clients, {} handles", client_count, handle_count);
            
            // Get all handles and demonstrate batch operations
            let handles = server_clone.get_all_client_handles().await;
            for handle in handles {
                if handle.is_connected().await {
                    let _ = handle.send_message(WsMessage::Text("Periodic ping from server".to_string())).await;
                }
            }
        }
    });

    // Start the server
    server.start().await.map_err(|e| format!("Server error: {}", e))?;

    Ok(())
}

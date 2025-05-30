use std::path::PathBuf;
use std::time::Duration;
use tokio::time::sleep;
use log::{info, error};
use ws_rs::server::{WsServer, WsServerConfig, ServerHandler, ClientId, WsMessage};

/// Demo handler that shows per-client data storage
struct DataDemoHandler;

impl ServerHandler for DataDemoHandler {
    fn on_connect(&self, client_id: ClientId, path_info: ws_rs::path_utils::PathInfo) {
        info!("Client {} connected to path: {}", client_id.0, path_info.original_path);
        info!("Extracted suffix: {:?}", path_info.suffix);
    }

    fn on_disconnect(&self, client_id: ClientId) {
        info!("Client {} disconnected", client_id.0);
    }

    fn on_message(&self, client_id: ClientId, message: WsMessage) -> Option<WsMessage> {
        match &message {
            WsMessage::Text(text) => {
                info!("Message from {}: {}", client_id.0, text);

                // Handle data commands
                if text.starts_with("STORE:") {
                    let parts: Vec<&str> = text.splitn(3, ':').collect();
                    if parts.len() == 3 {
                        let key = parts[1];
                        let value = parts[2];
                        info!("Client {} wants to store: {} = {}", client_id.0, key, value);
                        return Some(WsMessage::Text(format!("STORED:{}:{}", key, value)));
                    }
                } else if text.starts_with("READ:") {
                    let parts: Vec<&str> = text.splitn(2, ':').collect();
                    if parts.len() == 2 {
                        let key = parts[1];
                        info!("Client {} wants to read: {}", client_id.0, key);
                        return Some(WsMessage::Text(format!("DATA:{}:value_for_{}", key, client_id.0)));
                    }
                }

                Some(WsMessage::Text(format!("Echo: {}", text)))
            }
            WsMessage::Binary(data) => {
                info!("Binary from {}: {} bytes", client_id.0, data.len());
                Some(WsMessage::Binary(data.clone()))
            }
            WsMessage::Close => {
                info!("Close from {}", client_id.0);
                Some(WsMessage::Close)
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let config = WsServerConfig {
        addr: "127.0.0.1:9998".to_string(),
        cert_path: PathBuf::from("./crate_cert/a_cert.pem"),
        key_path: PathBuf::from("./crate_cert/a_key.pem"),
        ca_cert_path: PathBuf::from("./crate_cert/ca_cert.pem"),
        max_connections: 100,
        connection_timeout: 30,
        client_cert_required: true,
    };

    let handler = DataDemoHandler;
    let server = WsServer::new(config, handler)?;

    info!("Starting demo server with per-client data storage...");

    // Spawn a task to demonstrate data operations
    let server_clone = server.clone();
    tokio::spawn(async move {
        sleep(Duration::from_secs(2)).await;
        
        info!("Demo: Checking connected clients...");
        let clients = server_clone.client_list().await;
        info!("Connected clients: {:?}", clients);

        // If we have clients, demonstrate data operations
        for client_id in clients {
            info!("Demonstrating data operations for client: {}", client_id.0);

            // Write some test data for this client
            if let Err(e) = server_clone.write_client_text_data(&client_id, "name".to_string(), format!("Client_{}", client_id.0)).await {
                error!("Failed to write client data: {}", e);
            }

            if let Err(e) = server_clone.write_client_text_data(&client_id, "status".to_string(), "active".to_string()).await {
                error!("Failed to write client status: {}", e);
            }

            // Write binary data
            let binary_data = vec![0x01, 0x02, 0x03, client_id.0.parse::<u8>().unwrap_or(0)];
            if let Err(e) = server_clone.write_client_data(&client_id, "config".to_string(), binary_data).await {
                error!("Failed to write client binary data: {}", e);
            }

            // Read the data back
            match server_clone.read_client_text_data(&client_id, "name").await {
                Ok(Some(name)) => info!("Client {} name: {}", client_id.0, name),
                Ok(None) => info!("No name found for client {}", client_id.0),
                Err(e) => error!("Failed to read client name: {}", e),
            }

            // List all keys for this client
            match server_clone.list_client_data_keys(&client_id).await {
                Ok(keys) => info!("Client {} has keys: {:?}", client_id.0, keys),
                Err(e) => error!("Failed to list client keys: {}", e),
            }
        }
    });

    // Start the server
    server.start().await?;
    Ok(())
}

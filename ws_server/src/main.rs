use log::{error, info, warn};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::interval;
use ws_rs::server::{WsServer, WsServerConfig, WsMessage, ConnectionInterface};
use ws_rs::path_utils::{parse_ocpp_station_id, parse_api_path};

/// Enhanced connection handler with comprehensive data send/receive examples
async fn handle_connection_interface(interface: ConnectionInterface) {
    let client_id = interface.client_id().clone();
    let addr = interface.addr();
    let path_info = interface.path_info();

    info!("Client connected: {} from {}", client_id.0, addr);
    info!("  Raw path: {}", path_info.raw());
    info!("  Decoded path: {}", path_info.decoded());
    info!("  URL suffix: {}", interface.url_suffix());
    info!("  Decoded suffix: {}", interface.url_suffix_decoded());

    // Analyze connection type
    let connection_type = if path_info.is_root() {
        info!("  → Root path connection");
        "root"
    } else if let Some(station_id) = parse_ocpp_station_id(path_info) {
        info!("  → OCPP Charging Station ID: {}", station_id);
        if station_id.contains('|') {
            info!("    Station ID contains special characters (properly decoded)");
        }
        "ocpp"
    } else if let Some((version, endpoint)) = parse_api_path(path_info) {
        info!("  → API Connection - Version: {}, Endpoint: {}", version, endpoint);
        "api"
    } else {
        let segments = path_info.segments();
        if !segments.is_empty() {
            info!("  → Custom path - First segment: {}, Total segments: {}",
                  segments[0], segments.len());
        }
        "custom"
    };

    // Send welcome message based on connection type
    send_welcome_message(&interface, connection_type).await;

    // Start background tasks for this connection
    start_background_tasks(&interface).await;

    // Main message handling loop with enhanced examples
    handle_client_messages(&interface, connection_type).await;
}

/// Send welcome message based on connection type
async fn send_welcome_message(interface: &ConnectionInterface, connection_type: &str) {
    let client_id = interface.client_id();

    let welcome_msg = match connection_type {
        "ocpp" => {
            format!("Welcome OCPP Charging Station! Your station ID: {}",
                   interface.url_suffix_decoded().trim_start_matches("/ocpp/"))
        }
        "api" => {
            format!("Welcome API Client! Connected to: {}", interface.url_suffix())
        }
        "root" => {
            "Welcome! You're connected to the root path.".to_string()
        }
        _ => {
            format!("Welcome! Connected to custom path: {}", interface.url_suffix())
        }
    };

    if let Err(e) = interface.send_message(WsMessage::Text(welcome_msg)).await {
        error!("Failed to send welcome message to {}: {}", client_id.0, e);
    }

    // Send connection info as a structured message
    let info_msg = format!(
        "INFO: Client ID: {}, Address: {}, Path: {}, Type: {}",
        client_id.0,
        interface.addr(),
        interface.url_suffix_decoded(),
        connection_type
    );

    if let Err(e) = interface.send_message(WsMessage::Text(info_msg)).await {
        error!("Failed to send info message to {}: {}", client_id.0, e);
    }
}

/// Start background tasks for the connection
async fn start_background_tasks(interface: &ConnectionInterface) {
    // Task 1: Periodic heartbeat/ping
    let interface_ping = interface.clone();
    tokio::spawn(async move {
        let client_id = interface_ping.client_id().clone();
        let mut ping_interval = interval(Duration::from_secs(30));
        let mut ping_counter = 0;

        loop {
            ping_interval.tick().await;

            if !interface_ping.is_connected() {
                info!("Ping task stopping for disconnected client {}", client_id.0);
                break;
            }

            ping_counter += 1;
            let ping_msg = WsMessage::Text(format!("PING #{} from server", ping_counter));

            if let Err(e) = interface_ping.send_message(ping_msg).await {
                warn!("Failed to send ping to {}: {}", client_id.0, e);
                break;
            }

            info!("Sent ping #{} to client {}", ping_counter, client_id.0);
        }
    });

    // Task 2: Non-blocking message reader (demonstrates try_receive_message)
    let interface_reader = interface.clone();
    tokio::spawn(async move {
        let client_id_reader = interface_reader.client_id().clone();
        let mut check_interval = interval(Duration::from_millis(500));

        loop {
            check_interval.tick().await;

            if !interface_reader.is_connected() {
                info!("Non-blocking reader stopping for disconnected client {}", client_id_reader.0);
                break;
            }

            // Try to read messages without blocking
            while let Some(message) = interface_reader.try_receive_message().await {
                info!("Non-blocking reader got message from {}: {:?}", client_id_reader.0, message);

                // Process urgent messages here if needed
                if let WsMessage::Text(text) = &message {
                    if text.starts_with("URGENT:") {
                        let response = WsMessage::Text("URGENT message received and processed".to_string());
                        let _ = interface_reader.send_message(response).await;
                    }
                }
            }
        }
    });

    // Task 3: Status updates
    let interface_status = interface.clone();
    tokio::spawn(async move {
        let client_id_status = interface_status.client_id().clone();
        let mut status_interval = interval(Duration::from_secs(60));

        loop {
            status_interval.tick().await;

            if !interface_status.is_connected() {
                info!("Status task stopping for disconnected client {}", client_id_status.0);
                break;
            }

            let status_msg = WsMessage::Text(format!(
                "STATUS: Client {} still connected, path: {}",
                client_id_status.0,
                interface_status.url_suffix_decoded()
            ));

            if let Err(e) = interface_status.send_message(status_msg).await {
                warn!("Failed to send status to {}: {}", client_id_status.0, e);
                break;
            }
        }
    });
}

/// Handle client messages with comprehensive examples
async fn handle_client_messages(interface: &ConnectionInterface, connection_type: &str) {
    let client_id = interface.client_id().clone();

    info!("Starting message handler for {} client {}", connection_type, client_id.0);

    loop {
        match interface.receive_message().await {
            Some(message) => {
                match &message {
                    WsMessage::Text(text) => {
                        info!("Text message from {}: {}", client_id.0, text);

                        // Enhanced command processing
                        let response = process_text_command(text, interface, connection_type).await;

                        if let Err(e) = interface.send_message(response).await {
                            error!("Failed to send response to {}: {}", client_id.0, e);
                            break;
                        }
                    }
                    WsMessage::Binary(data) => {
                        info!("Binary message from {}: {} bytes", client_id.0, data.len());

                        // Process binary data
                        let response = process_binary_data(data, interface, connection_type).await;

                        if let Err(e) = interface.send_message(response).await {
                            error!("Failed to send binary response to {}: {}", client_id.0, e);
                            break;
                        }
                    }
                    WsMessage::Close => {
                        info!("Close request from {}", client_id.0);
                        let _ = interface.send_message(WsMessage::Close).await;
                        break;
                    }
                }
            }
            None => {
                // Connection closed
                info!("Client disconnected: {}", client_id.0);
                break;
            }
        }
    }
}

/// Process text commands with comprehensive examples
async fn process_text_command(text: &str, interface: &ConnectionInterface, connection_type: &str) -> WsMessage {
    let client_id = interface.client_id();

    // Parse command
    let trimmed = text.trim();

    match trimmed {
        // Basic commands
        "ping" => WsMessage::Text("pong".to_string()),
        "hello" => WsMessage::Text("Hello there!".to_string()),
        "time" => {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            WsMessage::Text(format!("Current timestamp: {}", now))
        }

        // Connection info commands
        "info" => {
            WsMessage::Text(format!(
                "Client Info - ID: {}, Address: {}, Path: {}, Type: {}",
                client_id.0,
                interface.addr(),
                interface.url_suffix_decoded(),
                connection_type
            ))
        }
        "status" => {
            let connected = interface.is_connected();
            WsMessage::Text(format!("Connection Status: {}", if connected { "Connected" } else { "Disconnected" }))
        }

        // OCPP-specific commands
        cmd if connection_type == "ocpp" && cmd.starts_with("ocpp:") => {
            let ocpp_cmd = cmd.strip_prefix("ocpp:").unwrap_or("");
            match ocpp_cmd {
                "heartbeat" => WsMessage::Text("OCPP Heartbeat Response: OK".to_string()),
                "status" => WsMessage::Text("OCPP Status: Available".to_string()),
                "start" => WsMessage::Text("OCPP Transaction Started".to_string()),
                "stop" => WsMessage::Text("OCPP Transaction Stopped".to_string()),
                _ => WsMessage::Text(format!("Unknown OCPP command: {}", ocpp_cmd))
            }
        }

        // API-specific commands
        cmd if connection_type == "api" && cmd.starts_with("api:") => {
            let api_cmd = cmd.strip_prefix("api:").unwrap_or("");
            match api_cmd {
                "version" => WsMessage::Text("API Version: 1.0".to_string()),
                "endpoints" => WsMessage::Text("Available endpoints: /data, /status, /config".to_string()),
                "health" => WsMessage::Text("API Health: OK".to_string()),
                _ => WsMessage::Text(format!("Unknown API command: {}", api_cmd))
            }
        }

        // Data simulation commands
        "simulate:data" => {
            WsMessage::Text(format!(
                "Simulated Data: {{\"temperature\": 25.5, \"humidity\": 60, \"timestamp\": {}, \"client\": \"{}\"}}",
                std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs(),
                client_id.0
            ))
        }
        "simulate:error" => {
            WsMessage::Text("ERROR: Simulated error condition detected".to_string())
        }
        "simulate:warning" => {
            WsMessage::Text("WARNING: Simulated warning condition".to_string())
        }

        // Bulk data commands
        "bulk:small" => {
            let data = (0..10).map(|i| format!("item_{}", i)).collect::<Vec<_>>().join(",");
            WsMessage::Text(format!("Bulk Data (small): [{}]", data))
        }
        "bulk:large" => {
            let data = (0..100).map(|i| format!("data_item_{:03}", i)).collect::<Vec<_>>().join(",");
            WsMessage::Text(format!("Bulk Data (large): [{}]", data))
        }

        // Help command
        "help" => {
            let help_text = match connection_type {
                "ocpp" => "Available commands: ping, hello, time, info, status, help, ocpp:heartbeat, ocpp:status, ocpp:start, ocpp:stop, simulate:data, simulate:error, bulk:small, bulk:large",
                "api" => "Available commands: ping, hello, time, info, status, help, api:version, api:endpoints, api:health, simulate:data, simulate:warning, bulk:small, bulk:large",
                _ => "Available commands: ping, hello, time, info, status, help, simulate:data, simulate:error, simulate:warning, bulk:small, bulk:large"
            };
            WsMessage::Text(help_text.to_string())
        }

        // Echo for everything else
        _ => WsMessage::Text(format!("Echo ({}): {}", connection_type, text))
    }
}

/// Process binary data with examples
async fn process_binary_data(data: &[u8], interface: &ConnectionInterface, connection_type: &str) -> WsMessage {
    let client_id = interface.client_id();

    info!("Processing {} bytes of binary data from {} ({})", data.len(), client_id.0, connection_type);

    // Analyze binary data
    if data.is_empty() {
        return WsMessage::Text("Error: Empty binary data received".to_string());
    }

    // Check for common patterns
    if data.len() >= 4 {
        let header = &data[0..4];
        match header {
            [0x89, 0x50, 0x4E, 0x47] => {
                // PNG header
                WsMessage::Text(format!("Detected PNG image ({} bytes)", data.len()))
            }
            [0xFF, 0xD8, 0xFF, _] => {
                // JPEG header
                WsMessage::Text(format!("Detected JPEG image ({} bytes)", data.len()))
            }
            [0x50, 0x4B, 0x03, 0x04] => {
                // ZIP header
                WsMessage::Text(format!("Detected ZIP archive ({} bytes)", data.len()))
            }
            _ => {
                // Generic binary data processing
                let checksum = data.iter().fold(0u32, |acc, &b| acc.wrapping_add(b as u32));

                info!("Binary data processed: {} bytes, checksum: 0x{:08X}, first 4 bytes: {:02X} {:02X} {:02X} {:02X}",
                    data.len(),
                    checksum,
                    data[0], data[1], data[2], data[3]
                );

                // Echo back the data with metadata
                let mut response = Vec::new();
                response.extend_from_slice(b"PROCESSED:");
                response.extend_from_slice(&data.len().to_le_bytes());
                response.extend_from_slice(&checksum.to_le_bytes());
                response.extend_from_slice(data);

                WsMessage::Binary(response)
            }
        }
    } else {
        // Small binary data
        let hex_data = data.iter().map(|b| format!("{:02X}", b)).collect::<Vec<_>>().join(" ");
        WsMessage::Text(format!("Small binary data ({} bytes): {}", data.len(), hex_data))
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

    // Create and initialize server
    info!("Starting WebSocket server...");
    let mut server = WsServer::new(config)
        .map_err(|e| format!("Failed to create server: {}", e))?;

    server.initialize().await
        .map_err(|e| format!("Failed to initialize server: {}", e))?;

    info!("Server ready with URL suffix extraction and connection interface functionality...");

    // Wrap server in Arc for shared access
    let server = Arc::new(server);

    // Start a background task to monitor connections
    let server_monitor = server.clone();
    tokio::spawn(async move {
        let mut monitor_interval = interval(Duration::from_secs(30));

        loop {
            monitor_interval.tick().await;

            let total_connections = server_monitor.connection_count().await;
            let client_ids = server_monitor.get_connected_client_ids().await;

            if total_connections > 0 {
                info!("📈 Connection Monitor: {} active connections", total_connections);
                for client_id in &client_ids {
                    if let Some(interface) = server_monitor.get_connection_interface(client_id).await {
                        info!("  👤 Client {}: {} ({})",
                              client_id.0,
                              interface.addr(),
                              interface.url_suffix_decoded());
                    }
                }
            } else {
                info!("📉 Connection Monitor: No active connections");
            }
        }
    });

    // Accept connections in a loop - supports multiple concurrent clients
    let mut connection_counter = 0;
    loop {
        match server.accept_connection().await {
            Ok(interface) => {
                connection_counter += 1;
                let current_total = server.connection_count().await;

                info!("✅ New connection #{} accepted from {}",
                      connection_counter, interface.addr());
                info!("📊 Total active connections: {}", current_total);

                // Spawn a task to handle this connection - each client runs independently
                tokio::spawn(async move {
                    info!("🚀 Starting handler for connection #{} ({})",
                          connection_counter, interface.client_id().0);
                    handle_connection_interface(interface).await;
                    info!("🔚 Handler finished for connection #{}", connection_counter);
                });

                // Log all connected client IDs for verification
                let client_ids = server.get_connected_client_ids().await;
                info!("🔗 Currently connected clients: {:?}",
                      client_ids.iter().map(|id| &id.0).collect::<Vec<_>>());
            }
            Err(e) => {
                error!("❌ Failed to accept connection: {}", e);
                // Small delay to avoid busy loop on persistent errors
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            }
        }
    }
}

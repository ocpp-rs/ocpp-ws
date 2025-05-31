use log::{error, info};
use std::time::Duration;
use tokio::time::sleep;
use ws_rs::client::{MessageType, WebSocketClient};

/// WebSocket client application that demonstrates the optimized client library features
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    env_logger::init();

    // Create WebSocket client with OCPP subprotocol and RFC 7692 compression support
    let compression_config = ws_rs::compression::CompressionConfig::new()
        .with_enabled(true) // Enable RFC 7692 permessage-deflate compression
        .with_level(6) // Balanced compression level (0=fastest, 9=best compression)
        .with_client_max_window_bits(Some(15)) // Request maximum compression window
        .with_server_max_window_bits(Some(15)) // Accept server maximum compression window
        .with_client_no_context_takeover(false) // Allow client context takeover for better compression
        .with_server_no_context_takeover(false); // Allow server context takeover for better compression

    info!("📊 Client Compression Configuration:");
    info!("   - Enabled: {}", compression_config.enabled);
    info!("   - Level: {} (0=fastest, 9=best compression)", compression_config.level);
    info!("   - Client max window bits: {:?}", compression_config.client_max_window_bits);
    info!("   - Server max window bits: {:?}", compression_config.server_max_window_bits);
    info!("   - Client no context takeover: {}", compression_config.client_no_context_takeover);
    info!("   - Server no context takeover: {}", compression_config.server_no_context_takeover);
    info!("📝 Note: Actual compression requires tungstenite library support (negotiation ready)");

    let mut client = WebSocketClient::builder()
        .with_connection_timeout(Duration::from_secs(10)) // Set connection timeout
        .with_auto_reconnect(true) // Enable auto-reconnection
        .with_max_reconnect_attempts(3) // Configure reconnection attempts
        .with_reconnect_delay(Duration::from_secs(2)) // Set delay between reconnections
        .with_channel_capacity(200) // Optimize channel buffer size
        .with_subprotocols(vec![
            "ocpp2.1".to_string(),
            "ocpp2.0.1".to_string(),
            "ocpp1.6".to_string()
        ]) // OCPP protocol versions as per OCPP 2.1 specification
        .with_compression(compression_config) // Enable RFC 7692 permessage-deflate compression
        .build();

    // Test different OCPP and WebSocket scenarios
    let test_urls = vec![
        ("wss://127.0.0.1:9999/ocpp/CS001", "OCPP Station CS001", vec!["hello"]),
        ("wss://127.0.0.1:9999/webServices/ocpp/CS3211", "OCPP-J Station CS3211 (OCPP 2.1 style)", vec![
            r#"[2,"19223201","BootNotification",{"reason":"PowerUp","chargingStation":{"model":"SingleSocketCharger","vendorName":"VendorX"}}]"#
        ]),
        ("wss://127.0.0.1:9999/ocpp/RDAM%7C123", "OCPP Station with special chars", vec!["status"]),
        ("wss://127.0.0.1:9999/", "Root path", vec!["ping"]),
    ];

    for (url, description, messages) in test_urls {
        info!("Testing: {} ({})", url, description);

        match client
            .connect(
                url,
                "./crate_cert",
                "b_cert.pem",
                "b_key.pem",
                "ca_cert.pem",
            )
            .await
        {
            Ok(_) => {
                info!("✓ Connected to: {}", url);

                // Send test message
                for message in messages {
                    if let Err(e) = client.send_text(message.to_string()).await {
                        error!("Failed to send message: {}", e);
                    } else {
                        info!("Sent: {}", message);

                        // Try to receive all responses
                        loop {
                            match client.receive_message_timeout(Duration::from_millis(500)).await {
                                Ok(Some(MessageType::Text(response))) => {
                                    info!("📨 Received: {}", response);
                                }
                                Ok(Some(MessageType::Binary(data))) => {
                                    info!("📦 Received binary: {} bytes - {:?}", data.len(), data);
                                }
                                Ok(None) => {
                                    info!("❌ Not connected");
                                    break;
                                }
                                Err(_) => {
                                    // Timeout - no more messages
                                    break;
                                }
                            }
                        }
                    }
                }

                // Small delay to allow server to send all messages
                sleep(Duration::from_millis(100)).await;

                // Close connection
                client.close().await;
                info!("Connection closed");
            }
            Err(e) => {
                error!("✗ Failed to connect to {}: {}", url, e);
            }
        }
    }

    info!("Testing completed!");
    Ok(())
}

use log::{error, info};
use std::time::Duration;
use tokio::time::sleep;
use ws_rs::client::{MessageType, WebSocketClient};

/// WebSocket client application that demonstrates the optimized client library features
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    env_logger::init();

    // Create WebSocket client with optimized configuration
    let mut client = WebSocketClient::builder()
        .with_connection_timeout(Duration::from_secs(10)) // Set connection timeout
        .with_auto_reconnect(true) // Enable auto-reconnection
        .with_max_reconnect_attempts(3) // Configure reconnection attempts
        .with_reconnect_delay(Duration::from_secs(2)) // Set delay between reconnections
        .with_channel_capacity(200) // Optimize channel buffer size
        .build();

    // Test different URL paths with automatic URL decoding and data operations
    let test_urls = vec![
        ("wss://127.0.0.1:9999/ocpp/CS001", "OCPP Charging Station CS001", vec![
            "Test message from OCPP Charging Station CS001",
            "STORE:station_CS001:Online",
            "READ:station_CS001",
        ]),
        ("wss://127.0.0.1:9999/ocpp/RDAM%7C123", "OCPP Station with pipe character (URL encoded)", vec![
            "Test message from OCPP Station with pipe character",
            "STORE:station_RDAM123:Charging",
            "READ:station_RDAM123",
        ]),
        ("wss://127.0.0.1:9999/api/v1/websocket", "API WebSocket v1", vec![
            "Test message from API WebSocket v1",
            "STORE:api_endpoint:active",
            "READ:api_endpoint",
        ]),
        ("wss://127.0.0.1:9999/", "Root path", vec![
            "Test message from Root path",
            "STORE:root_connection:established",
            "READ:root_connection",
        ]),
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

                // Send multiple test messages including data operations
                for message in messages {
                    if let Err(e) = client.send_text(message.to_string()).await {
                        error!("Failed to send message: {}", e);
                    } else {
                        info!("Sent: {}", message);

                        // Try to receive response
                        match client.receive_message_timeout(Duration::from_secs(2)).await {
                            Ok(Some(MessageType::Text(response))) => {
                                info!("Received: {}", response);
                            }
                            Ok(Some(MessageType::Binary(data))) => {
                                info!("Received binary: {} bytes", data.len());
                            }
                            Ok(None) => {
                                error!("Not connected");
                            }
                            Err(_) => {
                                error!("Receive timeout");
                            }
                        }
                    }

                    // Small delay between messages
                    sleep(Duration::from_millis(200)).await;
                }

                // Add small delay before closing to allow TLS cleanup
                sleep(Duration::from_millis(100)).await;

                // Close connection
                client.close().await;
                info!("Connection closed\n");
            }
            Err(e) => {
                error!("✗ Failed to connect to {}: {}\n", url, e);
            }
        }

        // Wait between tests
        sleep(Duration::from_secs(1)).await;
    }

    info!("URL path conversion testing completed!");
    Ok(())
}

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

    // Test a simple connection with minimal messages
    let test_urls = vec![
        ("wss://127.0.0.1:9999/ocpp/CS001", "OCPP Station", vec!["hello"]),
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

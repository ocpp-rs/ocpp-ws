use log::{error, info};
use std::time::Duration;
use tokio::time::sleep;
use ws_lib::client::{MessageType, WebSocketClient};

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

    // Connect to WebSocket server with proper error handling
    info!("Connecting to WebSocket server...");
    match client
        .connect(
            "wss://127.0.0.1:9999",
            "./crate_cert",
            "b_cert.pem",
            "b_key.pem",
            "ca_cert.pem",
        )
        .await
    {
        Ok(_) => info!("Connected to WebSocket server"),
        Err(e) => {
            error!("Connection failed: {}", e);
            return Err(e);
        }
    }

    // Verify connection is active
    if !client.is_connected() {
        error!("Connection status check failed");
        return Err("Connection status verification failed".into());
    }

    // Send a text message
    let message = "Hello, WebSocket server!";
    info!("Sending message: {}", message);
    client.send_text(message.to_string()).await?;

    // Receive message with timeout
    match client.receive_message_timeout(Duration::from_secs(5)).await {
        Ok(Some(response)) => match response {
            MessageType::Text(text) => info!("Received server response: {}", text),
            MessageType::Binary(data) => {
                info!("Received server binary response: {} bytes", data.len())
            }
        },
        Ok(None) => {
            error!("Not connected to server");
            return Err("Not connected to server".into());
        }
        Err(_) => {
            error!("Receive timeout");
            // Continue despite timeout
        }
    }

    // Send multiple messages
    for i in 1..=5 {
        // Create test message
        let message = format!("Message #{}", i);
        info!("Sending message: {}", message);

        // Send the message
        if let Err(e) = client.send_text(message).await {
            error!("Send failed: {}", e);

            // Check connection and attempt reconnect if needed
            if !client.check_connection().await {
                info!("Connection lost, attempting to reconnect");
                if let Err(e) = client.reconnect().await {
                    error!("Reconnection failed: {}", e);
                    break;
                }
                info!("Reconnection successful");
            }
            continue;
        }

        // Receive response with timeout
        match client.receive_message_timeout(Duration::from_secs(3)).await {
            Ok(Some(response)) => match response {
                MessageType::Text(text) => info!("Received server response: {}", text),
                MessageType::Binary(data) => {
                    info!("Received server binary response: {} bytes", data.len())
                }
            },
            Ok(None) => {
                error!("Not connected to server");
                break;
            }
            Err(_) => {
                error!("Receive timeout");
                // Send a ping to check connection
                if let Err(e) = client.ping().await {
                    error!("Ping failed: {}", e);
                    break;
                }
            }
        }

        // Wait between messages
        sleep(Duration::from_secs(1)).await;
    }

    // Gracefully close the connection
    info!("Closing WebSocket connection");
    client.close().await;

    Ok(())
}

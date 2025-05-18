use std::time::Duration;
use log::{error, info};
use tokio::time::sleep;
use ws_lib::client::{WebSocketClient, MessageType};

/// WebSocket client application that demonstrates the optimized client library features
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    env_logger::init();

    // Create WebSocket client with optimized configuration
    let mut client = WebSocketClient::builder()
        .with_connection_timeout(Duration::from_secs(10))  // Set connection timeout
        .with_auto_reconnect(true)                        // Enable auto-reconnection
        .with_max_reconnect_attempts(3)                   // Configure reconnection attempts
        .with_reconnect_delay(Duration::from_secs(2))     // Set delay between reconnections
        .with_channel_capacity(200)                       // Optimize channel buffer size
        .build();

    // Connect to WebSocket server with proper error handling
    info!("连接到WebSocket服务器...");
    match client.connect(
        "wss://127.0.0.1:9000",
        "./crate_cert",
        "b_cert.pem",
        "b_key.pem",
        "ca_cert.pem",
    ).await {
        Ok(_) => info!("已连接到WebSocket服务器"),
        Err(e) => {
            error!("连接失败: {}", e);
            return Err(e);
        }
    }

    // Verify connection is active
    if !client.is_connected() {
        error!("连接状态检查失败");
        return Err("Connection state verification failed".into());
    }

    // Send a text message
    let message = "你好，WebSocket服务器！";
    info!("发送消息: {}", message);
    client.send_text(message.to_string()).await?;

    // Receive message with timeout
    match client.receive_message_timeout(Duration::from_secs(5)).await {
        Ok(Some(response)) => match response {
            MessageType::Text(text) => info!("收到服务器响应: {}", text),
            MessageType::Binary(data) => info!("收到服务器二进制响应: {} 字节", data.len()),
        },
        Ok(None) => {
            error!("未连接到服务器");
            return Err("Not connected to server".into());
        },
        Err(_) => {
            error!("接收超时");
            // Continue despite timeout
        }
    }

    // Send multiple messages
    for i in 1..=5 {
        // Create test message
        let message = format!("消息 #{}", i);
        info!("发送消息: {}", message);
        
        // Send the message
        if let Err(e) = client.send_text(message).await {
            error!("发送失败: {}", e);
            
            // Check connection and attempt reconnect if needed
            if !client.check_connection().await {
                info!("连接丢失，尝试重连");
                if let Err(e) = client.reconnect().await {
                    error!("重连失败: {}", e);
                    break;
                }
                info!("重连成功");
            }
            continue;
        }

        // Receive response with timeout
        match client.receive_message_timeout(Duration::from_secs(3)).await {
            Ok(Some(response)) => match response {
                MessageType::Text(text) => info!("收到服务器响应: {}", text),
                MessageType::Binary(data) => info!("收到服务器二进制响应: {} 字节", data.len()),
            },
            Ok(None) => {
                error!("未连接到服务器");
                break;
            },
            Err(_) => {
                error!("接收超时");
                // Send a ping to check connection
                if let Err(e) = client.ping().await {
                    error!("Ping失败: {}", e);
                    break;
                }
            }
        }

        // Wait between messages
        sleep(Duration::from_secs(1)).await;
    }

    // Gracefully close the connection
    info!("关闭WebSocket连接");
    client.close().await;

    Ok(())
}

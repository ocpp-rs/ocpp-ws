# 客户端连接句柄系统使用指南

## 概述

客户端连接句柄系统允许应用层通过句柄（Handle）与特定的客户端连接进行双向通信：

1. **发送消息到客户端** - 使用 `handle.send_message()`
2. **读取客户端发送的消息** - 使用 `handle.read_message()` 或 `handle.try_read_message()`

## 核心组件

### ClientHandle

`ClientHandle` 是与特定客户端连接交互的主要接口：

```rust
pub struct ClientHandle {
    client_id: ClientId,
    addr: SocketAddr,
    path_info: PathInfo,
    tx: mpsc::Sender<WsMessage>,
    message_rx: Arc<tokio::sync::Mutex<mpsc::Receiver<WsMessage>>>,
}
```

### 主要方法

#### 获取客户端信息
```rust
// 获取客户端ID
let client_id = handle.client_id();

// 获取客户端地址
let addr = handle.addr();

// 获取路径信息
let path_info = handle.path_info();
```

#### 发送消息到客户端
```rust
// 发送文本消息
handle.send_message(WsMessage::Text("Hello!".to_string())).await?;

// 发送二进制消息
handle.send_message(WsMessage::Binary(vec![1, 2, 3, 4])).await?;

// 发送关闭消息
handle.send_message(WsMessage::Close).await?;
```

#### 读取客户端消息
```rust
// 阻塞读取消息（等待直到有消息或连接关闭）
while let Some(message) = handle.read_message().await {
    match message {
        WsMessage::Text(text) => {
            println!("收到文本: {}", text);
        }
        WsMessage::Binary(data) => {
            println!("收到二进制数据: {} 字节", data.len());
        }
        WsMessage::Close => {
            println!("客户端请求关闭连接");
            break;
        }
    }
}

// 非阻塞读取消息（立即返回）
if let Some(message) = handle.try_read_message().await {
    // 处理消息
}
```

#### 检查连接状态
```rust
// 检查客户端是否仍然连接
let is_connected = handle.is_connected().await;
```

## 使用示例

### 1. 获取客户端句柄

```rust
// 在服务器中获取特定客户端的句柄
if let Some(handle) = server.get_client_handle(&client_id).await {
    // 使用句柄与客户端通信
}

// 获取所有客户端句柄
let all_handles = server.get_all_client_handles().await;
for handle in all_handles {
    // 处理每个客户端
}
```

### 2. 双向通信示例

```rust
impl ServerHandler for MyHandler {
    fn on_connect_with_path(&self, client_id: ClientId, addr: SocketAddr, path_info: PathInfo) {
        if let Some(server) = &self.server {
            let server_clone = server.clone();
            let client_id_clone = client_id.clone();
            
            tokio::spawn(async move {
                if let Some(handle) = server_clone.get_client_handle(&client_id_clone).await {
                    // 发送欢迎消息
                    let _ = handle.send_message(WsMessage::Text("欢迎连接!".to_string())).await;
                    
                    // 启动消息读取循环
                    tokio::spawn(async move {
                        while let Some(message) = handle.read_message().await {
                            match message {
                                WsMessage::Text(text) => {
                                    println!("客户端 {} 发送: {}", client_id_clone.0, text);
                                    
                                    // 根据消息内容响应
                                    match text.as_str() {
                                        "ping" => {
                                            let _ = handle.send_message(WsMessage::Text("pong".to_string())).await;
                                        }
                                        "time" => {
                                            let now = std::time::SystemTime::now()
                                                .duration_since(std::time::UNIX_EPOCH)
                                                .unwrap()
                                                .as_secs();
                                            let response = format!("当前时间戳: {}", now);
                                            let _ = handle.send_message(WsMessage::Text(response)).await;
                                        }
                                        _ => {
                                            let echo = format!("回显: {}", text);
                                            let _ = handle.send_message(WsMessage::Text(echo)).await;
                                        }
                                    }
                                }
                                WsMessage::Binary(data) => {
                                    println!("客户端 {} 发送二进制数据: {} 字节", client_id_clone.0, data.len());
                                    // 回显二进制数据
                                    let _ = handle.send_message(WsMessage::Binary(data)).await;
                                }
                                WsMessage::Close => {
                                    println!("客户端 {} 请求关闭连接", client_id_clone.0);
                                    break;
                                }
                            }
                        }
                        println!("客户端 {} 的消息读取循环结束", client_id_clone.0);
                    });
                }
            });
        }
    }
}
```

### 3. 批量操作示例

```rust
// 向所有连接的客户端广播消息
async fn broadcast_message(server: &WsServer, message: &str) {
    let handles = server.get_all_client_handles().await;
    for handle in handles {
        if handle.is_connected().await {
            let _ = handle.send_message(WsMessage::Text(message.to_string())).await;
        }
    }
}

// 定期检查客户端状态
async fn monitor_clients(server: Arc<WsServer>) {
    loop {
        tokio::time::sleep(Duration::from_secs(30)).await;
        
        let handles = server.get_all_client_handles().await;
        let mut active_count = 0;
        
        for handle in handles {
            if handle.is_connected().await {
                active_count += 1;
                // 发送心跳消息
                let _ = handle.send_message(WsMessage::Text("heartbeat".to_string())).await;
            }
        }
        
        println!("活跃客户端数量: {}", active_count);
    }
}
```

## 注意事项

1. **线程安全**: `ClientHandle` 是 `Clone` 的，可以在多个任务之间安全共享
2. **消息顺序**: 每个客户端的消息按接收顺序转发到句柄
3. **内存管理**: 当客户端断开连接时，相关的句柄会自动清理
4. **错误处理**: 发送消息时如果客户端已断开，会返回错误
5. **背压控制**: 消息通道有缓冲区限制，避免内存无限增长

## 运行示例

```bash
# 编译并运行示例
cd ws-rs
cargo run --example client_handle_demo

# 使用 WebSocket 客户端连接测试
# 连接到: wss://127.0.0.1:9999/test/path
# 发送消息: "ping", "status", "info" 等
```

这个客户端句柄系统提供了一个简洁而强大的接口，让应用层可以轻松地与特定客户端进行双向通信，而无需直接处理底层的 WebSocket 连接管理。

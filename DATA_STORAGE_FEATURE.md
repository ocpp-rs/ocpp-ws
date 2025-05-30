# WebSocket 服务器基于客户端的数据存储功能

## 概述

我们为 WebSocket 服务器添加了完整的**基于客户端**的数据读写功能，每个客户端都有独立的数据存储空间。这个功能使用内存中的 HashMap 存储，支持文本和二进制数据，确保客户端之间的数据完全隔离。

## 功能特性

### 数据存储结构
- **存储类型**: 每个客户端独立的 `HashMap<String, Vec<u8>>`
- **线程安全**: 使用 `Arc<RwLock<HashMap<String, Vec<u8>>>>` 确保并发安全
- **数据隔离**: 每个客户端的数据完全独立，互不影响
- **数据类型**: 支持任意二进制数据和文本数据

### 核心接口

#### 1. 写入数据
```rust
// 写入二进制数据到指定客户端
pub async fn write_client_data(&self, client_id: &ClientId, key: String, data: Vec<u8>) -> Result<(), String>

// 写入文本数据到指定客户端（便捷方法）
pub async fn write_client_text_data(&self, client_id: &ClientId, key: String, text: String) -> Result<(), String>
```

#### 2. 读取数据
```rust
// 从指定客户端读取二进制数据
pub async fn read_client_data(&self, client_id: &ClientId, key: &str) -> Result<Option<Vec<u8>>, String>

// 从指定客户端读取文本数据（便捷方法）
pub async fn read_client_text_data(&self, client_id: &ClientId, key: &str) -> Result<Option<String>, String>
```

#### 3. 数据管理
```rust
// 删除指定客户端的数据
pub async fn delete_client_data(&self, client_id: &ClientId, key: &str) -> Result<bool, String>

// 获取指定客户端的所有键
pub async fn list_client_data_keys(&self, client_id: &ClientId) -> Result<Vec<String>, String>

// 清空指定客户端的所有数据
pub async fn clear_client_data(&self, client_id: &ClientId) -> Result<usize, String>
```

## 使用示例

### 服务器端使用

```rust
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 创建服务器
    let server = WsServer::new(config, handler)?;

    // 启动服务器
    let server_clone = server.clone();
    tokio::spawn(async move {
        // 等待客户端连接
        let clients = server_clone.client_list().await;

        for client_id in clients {
            // 为每个客户端写入数据
            server_clone.write_client_text_data(&client_id, "name".to_string(),
                format!("Client_{}", client_id.0)).await?;

            server_clone.write_client_text_data(&client_id, "status".to_string(),
                "active".to_string()).await?;

            // 写入二进制数据
            let binary_data = vec![0x01, 0x02, 0x03, 0x04];
            server_clone.write_client_data(&client_id, "config".to_string(), binary_data).await?;

            // 读取客户端数据
            if let Some(name) = server_clone.read_client_text_data(&client_id, "name").await? {
                println!("Client {} name: {}", client_id.0, name);
            }

            // 列出客户端的所有键
            let keys = server_clone.list_client_data_keys(&client_id).await?;
            println!("Client {} keys: {:?}", client_id.0, keys);
        }
    });

    server.start().await?;
    Ok(())
}
```

### 客户端协议示例

客户端可以通过 WebSocket 消息与服务器进行数据交互：

```
// 存储数据
发送: "STORE:key:value"
接收: "STORED:key:value"

// 读取数据  
发送: "READ:key"
接收: "DATA:key:value"
```

## 测试结果

### 服务器启动日志
```
[INFO] Starting WebSocket server...
[INFO] Demonstrating data storage functionality...
[INFO] Data written to key: server:name (21 bytes)
[INFO] Data written to key: server:version (5 bytes)
[INFO] Data written to key: config:binary (5 bytes)
[INFO] Server name from store: OCPP WebSocket Server
[INFO] Server version from store: 1.0.0
[INFO] Data store keys: ["server:name", "config:binary", "server:version"]
[INFO] WebSocket server started on 127.0.0.1:9999
```

### 客户端测试日志
```
[INFO] Testing: wss://127.0.0.1:9999/ocpp/CS001 (OCPP Charging Station CS001)
[INFO] ✓ Connected to: wss://127.0.0.1:9999/ocpp/CS001
[INFO] Sent: STORE:station:CS001:Online
[INFO] Received: STORED:station:CS001:Online
[INFO] Sent: READ:station:CS001
[INFO] Received: DATA:station:CS001:example_value
```

## 技术实现细节

### 线程安全
- 使用 `RwLock` 允许多个并发读取操作
- 写入操作获取独占锁
- 所有操作都是异步的，不会阻塞服务器

### 内存管理
- 数据存储在内存中，服务器重启后数据会丢失
- 支持任意大小的数据（受系统内存限制）
- 自动处理 UTF-8 文本编码/解码

### 错误处理
- 所有操作返回 `Result` 类型
- 详细的错误信息和日志记录
- 优雅处理并发访问冲突

## 应用场景

1. **OCPP 充电桩管理**: 存储充电桩状态、配置信息
2. **会话管理**: 存储客户端连接状态和会话数据
3. **配置缓存**: 缓存服务器配置和运行时参数
4. **临时数据存储**: 存储处理过程中的临时数据
5. **统计信息**: 收集和存储运行时统计数据

## 性能特点

- **高性能**: 内存存储，读写速度快
- **并发安全**: 支持多客户端同时访问
- **低延迟**: 异步操作，不阻塞其他请求
- **可扩展**: 易于扩展到持久化存储（如数据库）

## 未来扩展

1. **持久化存储**: 添加数据库后端支持
2. **数据过期**: 支持 TTL（生存时间）
3. **数据压缩**: 对大数据进行压缩存储
4. **数据同步**: 多服务器实例间的数据同步
5. **访问控制**: 基于客户端的数据访问权限控制

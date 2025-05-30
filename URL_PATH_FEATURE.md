# WebSocket URL Path Extraction & Auto-Conversion Feature

## 概述

本功能为WebSocket服务器添加了URL路径提取和自动转换能力，允许服务端在连接建立时获取客户端请求的URL路径后缀，并自动进行URL解码和路径分析。这对于OCPP（开放充电点协议）等需要在URL中包含设备标识符的应用场景特别有用。

## 功能特性

- **URL路径提取**：在WebSocket握手过程中提取完整的URL路径
- **自动URL解码**：自动解码URL编码的字符（如`%7C`→`|`，`%20`→空格）
- **后缀处理**：自动去除路径前缀的`/`，提供干净的后缀字符串
- **路径分析**：提供路径段分割、OCPP站点ID解析、API路径解析等功能
- **兼容性**：保持与现有WebSocket客户端的完全兼容性
- **增强接口**：提供新的`on_connect_with_path`方法，同时保持原有接口向后兼容

## 使用示例

### 服务端实现

#### 使用增强的路径处理功能（推荐）

```rust
use ws_rs::server::{ServerHandler, ClientId, WsMessage};
use ws_rs::path_utils::{PathInfo, parse_ocpp_station_id, parse_api_path};
use std::net::SocketAddr;
use log::info;

struct MyHandler;

impl ServerHandler for MyHandler {
    // 实现原有接口以保持向后兼容
    fn on_connect(&self, client_id: ClientId, addr: SocketAddr, path: String) {
        info!("客户端连接: {} 来自 {} 路径: {}", client_id.0, addr, path);
    }

    // 使用增强的路径处理功能
    fn on_connect_with_path(&self, client_id: ClientId, addr: SocketAddr, path_info: PathInfo) {
        info!("客户端连接: {} 来自 {}", client_id.0, addr);
        info!("  原始路径: {}", path_info.raw());
        info!("  解码路径: {}", path_info.decoded());
        info!("  URL后缀: {}", path_info.decoded_suffix());

        // 检查是否为根路径连接
        if path_info.is_root() {
            info!("  → 根路径连接");
            return;
        }

        // 解析OCPP充电桩ID
        if let Some(station_id) = parse_ocpp_station_id(&path_info) {
            info!("  → OCPP充电桩连接，站点ID: {}", station_id);

            // 处理特殊字符（已自动解码）
            if station_id.contains('|') {
                info!("    站点ID包含特殊字符（已正确解码）");
            }

            // 在这里添加OCPP特定的逻辑
            // 例如：验证站点ID、建立充电会话等
        }
        // 解析API路径
        else if let Some((version, endpoint)) = parse_api_path(&path_info) {
            info!("  → API连接 - 版本: {}, 端点: {}", version, endpoint);

            // 在这里添加API特定的逻辑
            // 例如：验证API版本、建立API会话等
        }
        // 处理其他路径类型
        else {
            let segments = path_info.segments();
            if !segments.is_empty() {
                info!("  → 自定义路径 - 首段: {}, 总段数: {}",
                      segments[0], segments.len());
            }
        }
    }

    fn on_disconnect(&self, client_id: ClientId) {
        info!("客户端断开: {}", client_id.0);
    }

    fn on_message(&self, client_id: ClientId, message: WsMessage) -> Option<WsMessage> {
        // 处理消息...
        None
    }

    fn on_error(&self, client_id: Option<ClientId>, error: String) {
        // 处理错误...
    }
}
```

#### 传统方式（仍然支持）

```rust
impl ServerHandler for MyHandler {
    fn on_connect(&self, client_id: ClientId, addr: SocketAddr, path: String) {
        info!("客户端连接: {} 来自 {} 路径: {}", client_id.0, addr, path);

        // 手动提取URL后缀
        let suffix = if path.starts_with('/') {
            &path[1..]
        } else {
            &path
        };

        // 手动处理URL解码（不推荐，建议使用PathInfo）
        // ...
    }

    // 其他方法...
}
```

### 客户端连接示例

```rust
use ws_rs::client::WebSocketClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = WebSocketClient::new();
    
    // 连接到带有OCPP路径的WebSocket服务器
    client.connect(
        "wss://csms.example.com:9999/ocpp/CS001",  // 包含充电桩ID的URL
        "./certs",
        "client_cert.pem",
        "client_key.pem", 
        "ca_cert.pem"
    ).await?;
    
    // 发送消息...
    
    Ok(())
}
```

## 支持的URL格式与自动转换

### OCPP示例
- `wss://csms.example.com/ocpp/CS001`
  - 原始后缀: `ocpp/CS001`
  - 解码后缀: `ocpp/CS001` (无变化)
  - 充电桩ID: `CS001`

- `wss://csms.example.com/ocpp/RDAM%7C123`
  - 原始后缀: `ocpp/RDAM%7C123`
  - 解码后缀: `ocpp/RDAM|123` (自动解码`%7C`→`|`)
  - 充电桩ID: `RDAM|123`

- `wss://csms.example.com/ocpp/Station%20Name%20With%20Spaces`
  - 原始后缀: `ocpp/Station%20Name%20With%20Spaces`
  - 解码后缀: `ocpp/Station Name With Spaces` (自动解码`%20`→空格)
  - 充电桩ID: `Station Name With Spaces`

### API示例
- `wss://api.example.com/api/v1/websocket`
  - 原始后缀: `api/v1/websocket`
  - 解码后缀: `api/v1/websocket` (无变化)
  - API版本: `v1`, 端点: `websocket`

- `wss://api.example.com/api/v2/notifications`
  - 原始后缀: `api/v2/notifications`
  - 解码后缀: `api/v2/notifications` (无变化)
  - API版本: `v2`, 端点: `notifications`

### 自定义路径
- `wss://example.com/custom/path%2Fwith%2Fslashes`
  - 原始后缀: `custom/path%2Fwith%2Fslashes`
  - 解码后缀: `custom/path/with/slashes` (自动解码`%2F`→`/`)
  - 路径段: `["custom", "path", "with", "slashes"]`

### 根路径
- `wss://example.com/` → 后缀: `` (空字符串)
- `wss://example.com` → 后缀: `` (空字符串)

## 技术实现

### 核心组件

1. **PathInfo结构体**：
   ```rust
   pub struct PathInfo {
       pub raw_path: String,        // 原始路径
       pub decoded_path: String,    // 解码后的路径
       pub suffix: String,          // 后缀（去除前导'/'）
       pub decoded_suffix: String,  // 解码后的后缀
   }
   ```

2. **URL解码功能**：
   ```rust
   pub fn decode_url_path(path: &str) -> String {
       match percent_decode_str(path).decode_utf8() {
           Ok(decoded) => decoded.to_string(),
           Err(_) => path.to_string(), // 解码失败时返回原始字符串
       }
   }
   ```

3. **路径解析器**：
   - `parse_ocpp_station_id()` - 解析OCPP充电桩ID
   - `parse_api_path()` - 解析API版本和端点
   - `segments()` - 将路径分割为段

### 服务端变更

1. **ServerHandler trait增强**：
   - 保留原有`on_connect`方法（向后兼容）
   - 新增`on_connect_with_path`方法（增强功能）
   - 默认实现：新方法调用原方法

2. **WebSocket握手处理**：
   - 使用`accept_hdr_async`替代`accept_async`
   - 实现回调函数提取HTTP请求的URI路径
   - 创建`PathInfo`实例进行自动转换

3. **路径提取与转换逻辑**：
   ```rust
   let callback = move |request: &Request, response: Response| -> Result<Response, ErrorResponse> {
       let path = request.uri().path().to_string();
       // 存储路径信息...
       Ok(response)
   };

   // 创建PathInfo并自动转换
   let path_info = PathInfo::new(extracted_path);
   handler.on_connect_with_path(client_id, addr, path_info);
   ```

### 客户端兼容性

客户端代码无需任何修改，只需在连接URL中包含所需的路径即可：

```rust
// 原有方式（仍然支持）
client.connect("wss://example.com:9999", ...);

// 新方式（带路径）
client.connect("wss://example.com:9999/ocpp/CS001", ...);
```

## 测试验证

项目包含完整的测试用例，验证了以下场景：

### 单元测试
```bash
# 运行路径处理相关的单元测试
cargo test path_utils
```

测试覆盖：
1. **URL解码功能**：`%7C`→`|`，`%20`→空格，`%2F`→`/`
2. **路径后缀提取**：去除前导`/`
3. **PathInfo结构**：原始路径、解码路径、后缀处理
4. **OCPP解析**：充电桩ID提取
5. **API解析**：版本和端点提取
6. **路径段分割**：多级路径处理

### 集成测试
```bash
# 启动服务器
RUST_LOG=info cargo run --bin ws_server

# 在另一个终端运行客户端测试
RUST_LOG=info cargo run --bin ws_client
```

测试场景：
1. **OCPP路径**：
   - `/ocpp/CS001` - 基本充电桩ID
   - `/ocpp/RDAM%7C123` - 包含特殊字符的ID（URL编码）
   - `/ocpp/Station%20Name%20With%20Spaces` - 包含空格的ID

2. **API路径**：
   - `/api/v1/websocket` - API WebSocket连接
   - `/api/v2/notifications` - API通知端点

3. **自定义路径**：
   - `/custom/path%2Fwith%2Fslashes` - 包含编码斜杠的路径

4. **根路径**：`/` - 根路径连接

### 测试结果示例
```
[INFO] Client connected: client-xxx from 127.0.0.1:xxxxx
[INFO]   Raw path: /ocpp/RDAM%7C123
[INFO]   Decoded path: /ocpp/RDAM|123
[INFO]   URL suffix: ocpp/RDAM%7C123
[INFO]   Decoded suffix: ocpp/RDAM|123
[INFO]   → OCPP Charging Station ID: RDAM|123
[INFO]     Station ID contains special characters (properly decoded)
```

## 注意事项

1. **自动URL解码**：系统会自动解码URL编码字符，解码后的字符串可直接使用
2. **解码失败处理**：如果URL解码失败，系统会返回原始字符串并记录警告日志
3. **路径验证**：建议在应用层对提取的路径进行验证和清理
4. **安全考虑**：避免直接使用路径信息进行文件系统操作，防止路径遍历攻击
5. **性能影响**：路径提取和解码过程对连接建立性能影响微乎其微
6. **向后兼容**：原有的`on_connect`方法仍然可用，新功能通过`on_connect_with_path`提供

## 兼容性

- **向后兼容**：现有不使用路径的客户端仍可正常工作
- **Rust版本**：需要Rust 2024 edition
- **依赖库**：基于tokio-tungstenite 0.21.0

## 应用场景

1. **OCPP充电桩管理**：在URL中包含充电桩标识符
2. **多租户WebSocket服务**：根据路径区分不同租户
3. **API版本控制**：通过路径指定API版本
4. **设备管理**：在IoT场景中标识不同设备

# OCPP WebSocket 安全工具包

<p align="center">
  <img src="https://img.shields.io/badge/language-rust-orange" alt="Language">
  <img src="https://img.shields.io/badge/license-MIT-blue" alt="License">
  <img src="https://img.shields.io/badge/version-0.1.0-green" alt="Version">
</p>

[English Documentation](./README.md)

一个用于安全WebSocket通信的综合工具包，提供TLS支持，专为OCPP（开放充电点协议）实现而设计。

## 组件

本项目包含以下主要组件：

- **ws-rs**: 核心WebSocket库，提供TLS支持
- **crate_cert**: 证书生成工具
- **cert_check**: 证书验证工具
- **ws_server**: WebSocket服务器示例实现
- **ws_client**: WebSocket客户端示例实现

## 快速开始

### 1. 生成证书

首先需要生成必要的证书：

```bash
cd crate_cert && python main.py
```

注意：需要配置Python环境并安装依赖（`pip install -r requirements.txt`）

### 2. 验证证书

可以使用证书验证工具检查证书的有效性：

```bash
RUST_LOG=info cargo run -p cert_check
```

### 3. 启动WebSocket服务器

```bash
RUST_LOG=info cargo run -p ws_server
```

服务器将在127.0.0.2:8080上启动，等待客户端连接。

### 4. 运行WebSocket客户端

在另一个终端中：

```bash
RUST_LOG=info cargo run -p ws_client
```

## 日志系统

项目使用`log`和`env_logger`库实现日志功能。通过设置环境变量`RUST_LOG`控制日志级别：

- `error`: 仅显示错误信息
- `warn`: 显示警告和错误
- `info`: 显示一般信息（默认）
- `debug`: 显示调试信息
- `trace`: 显示所有日志

设置方法：
- Linux/macOS: `export RUST_LOG=debug`
- PowerShell: `$env:RUST_LOG="debug"`
- CMD: `set RUST_LOG=debug`

## 证书文件

运行WebSocket服务器和客户端前，确保已生成以下证书文件：

- `certs/ca_cert.pem`: CA证书
- `certs/a_cert.pem`: 客户端证书
- `certs/a_key.pem`: 客户端私钥
- `certs/b_cert.pem`: 服务器证书
- `certs/b_key.pem`: 服务器私钥

## 安全特性

- TLS加密通信保障数据传输安全
- 双向证书认证(mTLS)确保双方身份可信
- 加密通信防止中间人攻击
- 完整的证书验证机制确保只有可信实体能建立连接

## 注意事项

1. 如需添加127.0.0.2地址：
   - Windows: `netsh interface ipv4 add address "Loopback" 127.0.0.2 255.0.0.0`
   - Linux: `sudo ip addr add 127.0.0.2/8 dev lo`

2. 如需在不同机器上运行，请相应修改IP地址配置和证书设置

3. 项目使用Rust 2024 edition，请确保Rust工具链为最新版本

## 使用ws-rs库

核心功能以库的形式提供。在您的项目中包含它：

```toml
[dependencies]
ws-rs = { version = "0.1.0", features = ["client", "server"] }
```

### 服务器示例

```rust
use ws_rs::server::{WebSocketServer, ServerConfig};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = ServerConfig {
        address: "127.0.0.2:8080".to_string(),
        cert_path: PathBuf::from("certs/b_cert.pem"),
        key_path: PathBuf::from("certs/b_key.pem"),
        ca_cert_path: PathBuf::from("certs/ca_cert.pem"),
    };
    
    let server = WebSocketServer::new(config);
    server.run().await?;
    
    Ok(())
}
```

### 客户端示例

```rust
use ws_rs::client::{WebSocketClient, ClientConfig};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = ClientConfig {
        url: "wss://127.0.0.2:8080".to_string(),
        cert_path: PathBuf::from("certs/a_cert.pem"),
        key_path: PathBuf::from("certs/a_key.pem"),
        ca_cert_path: PathBuf::from("certs/ca_cert.pem"),
    };
    
    let client = WebSocketClient::new(config);
    let connection = client.connect().await?;
    
    // 使用连接...
    
    Ok(())
}
```

## 许可证

本项目采用MIT许可证 - 详情请参阅LICENSE文件。

## 贡献

欢迎贡献！请随时提交Pull Request。
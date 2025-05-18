# ws-rs

<p align="center">
  <img src="https://img.shields.io/badge/language-rust-orange" alt="Language">
  <img src="https://img.shields.io/badge/license-MIT-blue" alt="License">
  <img src="https://img.shields.io/badge/version-0.1.0-green" alt="Version">
</p>

[English Documentation](./README.md)

一个基于Rust构建的安全WebSocket库，为通信提供TLS支持。

## 特性

- **TLS支持** - 通过TLS加密和证书验证确保通信安全
- **双向TLS认证(mTLS)** - 客户端和服务器相互认证
- **异步架构** - 基于Tokio和Tokio-Tungstenite构建，提供高性能
- **灵活API** - 简单但功能强大的API，适用于客户端和服务器实现

## 安装

在您的`Cargo.toml`中添加：

```toml
[dependencies]
ws-rs = { version = "0.1.0", features = ["client", "server"] }
```

## 使用方法

### 服务器示例

```rust
use ws_rs::server::{WebSocketServer, ServerConfig};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化日志
    env_logger::init();

    // 配置服务器
    let config = ServerConfig {
        address: "127.0.0.2:8080".to_string(),
        cert_path: PathBuf::from("certs/b_cert.pem"),
        key_path: PathBuf::from("certs/b_key.pem"),
        ca_cert_path: PathBuf::from("certs/ca_cert.pem"),
    };

    // 创建并运行服务器
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
    // 初始化日志
    env_logger::init();

    // 配置客户端
    let config = ClientConfig {
        url: "wss://127.0.0.2:8080".to_string(),
        cert_path: PathBuf::from("certs/a_cert.pem"),
        key_path: PathBuf::from("certs/a_key.pem"),
        ca_cert_path: PathBuf::from("certs/ca_cert.pem"),
    };

    // 连接到服务器
    let client = WebSocketClient::new(config);
    let connection = client.connect().await?;

    // 使用连接
    // ...

    Ok(())
}
```

## 证书设置

该库需要TLS证书以实现安全通信。您可以使用配套的`crate_cert`工具生成测试证书：

```bash
cd ../crate_cert && python main.py
```

生成的证书应放置在`certs`目录中，结构如下：

```
certs/
  |- ca_cert.pem    # CA证书
  |- a_cert.pem     # 客户端证书
  |- a_key.pem      # 客户端私钥
  |- b_cert.pem     # 服务器证书
  |- b_key.pem      # 服务器私钥
```

## 日志系统

此库使用`log`crate进行日志记录。要启用日志记录，请配置一个实现，如`env_logger`：

```rust
// 使用默认设置初始化
env_logger::init();

// 或者指定日志级别
std::env::set_var("RUST_LOG", "info");
env_logger::init();
```

使用`RUST_LOG`环境变量控制日志级别：

- `error`: 仅显示错误信息
- `warn`: 显示警告和错误
- `info`: 显示一般信息（默认）
- `debug`: 显示调试信息
- `trace`: 显示所有日志

## 安全注意事项

1. 在生产环境中始终验证证书
2. 妥善保护私钥
3. 针对特定安全要求使用自定义证书验证
4. 对关键应用程序考虑证书吊销检查

## 许可证

本项目采用MIT许可证 - 详情请参阅LICENSE文件。

## 贡献

欢迎贡献！请随时提交Pull Request。

1. Fork此仓库
2. 创建您的特性分支 (`git checkout -b feature/amazing-feature`)
3. 提交您的更改 (`git commit -m '添加某些特性'`)
4. 推送到分支 (`git push origin feature/amazing-feature`)
5. 打开Pull Request

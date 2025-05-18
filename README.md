# OCPP WebSocket Security Toolkit

<p align="center">
  <img src="https://img.shields.io/badge/language-rust-orange" alt="Language">
  <img src="https://img.shields.io/badge/license-MIT-blue" alt="License">
  <img src="https://img.shields.io/badge/version-0.1.0-green" alt="Version">
</p>

[中文文档](./README_zh.md)

A comprehensive toolkit for secure WebSocket communications with TLS support, specifically designed for OCPP (Open Charge Point Protocol) implementations.

## Components

This project consists of the following main components:

- **ws-rs**: Core WebSocket library with TLS support
- **crate_cert**: Certificate generation tool
- **cert_check**: Certificate validation tool
- **ws_server**: Example WebSocket server implementation
- **ws_client**: Example WebSocket client implementation

## Quick Start

### 1. Generate Certificates

First, generate the necessary certificates:

```bash
cd crate_cert && python main.py
```

Note: Python environment and dependencies are required (`pip install -r requirements.txt`)

### 2. Verify Certificates

You can use the certificate validation tool to check certificate validity:

```bash
RUST_LOG=info cargo run -p cert_check
```

### 3. Start WebSocket Server

```bash
RUST_LOG=info cargo run -p ws_server
```

The server will start on 127.0.0.2:8080, waiting for client connections.

### 4. Run WebSocket Client

In another terminal:

```bash
RUST_LOG=info cargo run -p ws_client
```

## Logging System

The project uses the `log` and `env_logger` libraries for logging. Control log levels via the `RUST_LOG` environment variable:

- `error`: Only errors
- `warn`: Warnings and errors
- `info`: General information (default)
- `debug`: Debug information
- `trace`: All logs

Setting method:
- Linux/macOS: `export RUST_LOG=debug`
- PowerShell: `$env:RUST_LOG="debug"`
- CMD: `set RUST_LOG=debug`

## Certificate Files

Before running the WebSocket server and client, ensure you have generated the following certificate files:

- `certs/ca_cert.pem`: CA certificate
- `certs/a_cert.pem`: Client certificate
- `certs/a_key.pem`: Client private key
- `certs/b_cert.pem`: Server certificate
- `certs/b_key.pem`: Server private key

## Security Features

- TLS encrypted communication ensures data transmission security
- Mutual certificate authentication (mTLS) ensures both parties' identity is trusted
- Encrypted communication prevents man-in-the-middle attacks
- Complete certificate validation mechanism ensures only trusted entities can establish connections

## Notes

1. To add the 127.0.0.2 address:
   - Windows: `netsh interface ipv4 add address "Loopback" 127.0.0.2 255.0.0.0`
   - Linux: `sudo ip addr add 127.0.0.2/8 dev lo`

2. If you need to run on different machines, please modify the IP address configuration and certificate settings accordingly.

3. The project uses Rust 2024 edition, please ensure your Rust toolchain is up to date.

## Using the ws-rs Library

The core functionality is available as a library. Include it in your project:

```toml
[dependencies]
ws-rs = { version = "0.1.0", features = ["client", "server"] }
```

### Server Example

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

### Client Example

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
    
    // Use the connection...
    
    Ok(())
}
```

## License

This project is licensed under the MIT License - see the LICENSE file for details.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.
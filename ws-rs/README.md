# ws-rs

<p align="center">
  <img src="https://img.shields.io/badge/language-rust-orange" alt="Language">
  <img src="https://img.shields.io/badge/license-MIT-blue" alt="License">
  <img src="https://img.shields.io/badge/version-0.1.0-green" alt="Version">
</p>

[中文文档](./README_zh.md)

A secure WebSocket library for OCPP communications with TLS support, built in Rust.

## Features

- **TLS Support** - Secure communications with TLS encryption and certificate validation
- **Mutual TLS (mTLS)** - Client and server mutual authentication
- **Async Architecture** - Built on Tokio and Tokio-Tungstenite for high performance
- **Flexible API** - Simple yet powerful API for both client and server implementations

## Installation

Add the following to your `Cargo.toml`:

```toml
[dependencies]
ws-rs = { version = "0.1.0", features = ["client", "server"] }
```

## Usage

### Server Example

```rust
use ws_rs::server::{WebSocketServer, ServerConfig};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logger
    env_logger::init();
    
    // Configure server
    let config = ServerConfig {
        address: "127.0.0.2:8080".to_string(),
        cert_path: PathBuf::from("certs/b_cert.pem"),
        key_path: PathBuf::from("certs/b_key.pem"),
        ca_cert_path: PathBuf::from("certs/ca_cert.pem"),
    };
    
    // Create and run server
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
    // Initialize logger
    env_logger::init();
    
    // Configure client
    let config = ClientConfig {
        url: "wss://127.0.0.2:8080".to_string(),
        cert_path: PathBuf::from("certs/a_cert.pem"),
        key_path: PathBuf::from("certs/a_key.pem"),
        ca_cert_path: PathBuf::from("certs/ca_cert.pem"),
    };
    
    // Connect to server
    let client = WebSocketClient::new(config);
    let connection = client.connect().await?;
    
    // Use the connection
    // ...
    
    Ok(())
}
```

## Certificate Setup

This library expects TLS certificates for secure communication. You can use the companion `crate_cert` tool to generate test certificates:

```bash
cd ../crate_cert && python main.py
```

The generated certificates should be placed in a `certs` directory with the following structure:

```
certs/
  |- ca_cert.pem    # CA certificate
  |- a_cert.pem     # Client certificate
  |- a_key.pem      # Client private key
  |- b_cert.pem     # Server certificate
  |- b_key.pem      # Server private key
```

## Logging

This library uses the `log` crate for logging. To enable logging, configure an implementation such as `env_logger`:

```rust
// Initialize with default settings
env_logger::init();

// Or with a specific log level
std::env::set_var("RUST_LOG", "info");
env_logger::init();
```

Control log levels using the `RUST_LOG` environment variable:

- `error`: Only errors
- `warn`: Warnings and errors
- `info`: General information (default)
- `debug`: Debug information
- `trace`: All logs

## Security Notes

1. Always validate certificates in production environments
2. Protect private keys appropriately
3. Use custom certificate validation for specific security requirements
4. Consider certificate revocation checking for critical applications

## License

This project is licensed under the MIT License - see the LICENSE file for details.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add some amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request
# WebSocket双向证书认证示例

这个项目包含两个组件：
- `ws_server`: WebSocket服务器，使用127.0.0.2地址和第二个证书
- `ws_client`: WebSocket客户端，使用127.0.0.1地址和第一个证书

两个组件都使用TLS进行加密，并且使用双向证书认证（mTLS）来验证对方的身份。

## 前提条件

在运行这个示例之前，你需要先生成证书：

```bash
# 生成证书和CSR
cargo run -p cert_gen

# 使用CA签名证书
cargo run -p ca
```

这将生成以下文件：
- `certs/ca_cert.pem`: CA证书
- `certs/cert1.pem`: 第一个证书（用于客户端）
- `certs/cert1_key.pem`: 第一个证书的私钥
- `certs/cert2.pem`: 第二个证书（用于服务器）
- `certs/cert2_key.pem`: 第二个证书的私钥

## 运行服务器

```bash
# 设置日志级别（可选）
$env:RUST_LOG="info"  # PowerShell
# 或
set RUST_LOG=info     # CMD
# 或
export RUST_LOG=info  # Bash

# 运行服务器
cargo run -p ws_server
```

服务器将在127.0.0.2:8080上启动，并等待客户端连接。

## 运行客户端

在另一个终端中：

```bash
# 设置日志级别（可选）
$env:RUST_LOG="info"  # PowerShell
# 或
set RUST_LOG=info     # CMD
# 或
export RUST_LOG=info  # Bash

# 运行客户端
cargo run -p ws_client
```

客户端将连接到服务器，发送几条消息，然后关闭连接。

## 注意事项

1. 确保在运行客户端和服务器之前已经生成了证书。
2. 如果你的系统没有127.0.0.2地址，你可能需要添加它：
   - Windows: `netsh interface ipv4 add address "Loopback" 127.0.0.2 255.0.0.0`
   - Linux: `sudo ip addr add 127.0.0.2/8 dev lo`
3. 如果你想在不同的机器上运行客户端和服务器，需要修改IP地址和证书配置。

## 功能说明

- 服务器和客户端都使用TLS加密通信。
- 服务器要求客户端提供证书，并验证证书是否由受信任的CA签名。
- 客户端也验证服务器的证书是否由受信任的CA签名。
- 客户端发送消息，服务器回复确认消息。
- 所有的通信都是加密的，并且双方都验证了对方的身份。

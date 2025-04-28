# 证书验证与WebSocket安全通信工具

这个项目包含三个主要组件：
- `cert_check`: 证书验证工具，用于检查证书的有效性和信任关系
- `ws_server`: 基于WebSocket的安全服务器，使用TLS和双向证书认证
- `ws_client`: 基于WebSocket的安全客户端，使用TLS和双向证书认证

## 日志系统

项目使用`log`和`env_logger`库实现日志功能。可以通过设置环境变量`RUST_LOG`来控制日志级别：

- `error`: 只显示错误信息
- `warn`: 显示警告和错误信息
- `info`: 显示一般信息、警告和错误信息（默认）
- `debug`: 显示调试信息及以上级别
- `trace`: 显示所有日志信息

### 使用方法


在Linux/macOS中：

```bash
cd crate_cert && python main.py #需要配置python环境
RUST_LOG=debug cargo run -p cert_check  # 或 ws_server 或 ws_client
RUST_LOG=debug cargo run -p ws_server
RUST_LOG=debug cargo run -p ws_client
```

## 证书验证工具

使用证书验证工具检查证书的有效性和信任关系：

```bash
cargo run -p cert_check
```

这将验证证书是否由受信任的CA签名，并检查它们的有效性。

## WebSocket安全通信

项目实现了基于WebSocket的安全通信，使用TLS加密和双向证书认证（mTLS）。

### 前提条件

在运行WebSocket服务器和客户端之前，确保已经生成了必要的证书文件：
- `certs/ca_cert.pem`: CA证书
- `certs/a_cert.pem`: 客户端证书
- `certs/a_key.pem`: 客户端私钥
- `certs/b_cert.pem`: 服务器证书
- `certs/b_key.pem`: 服务器私钥

### 运行服务器

```bash
# 设置日志级别（可选）
export RUST_LOG=info  # Linux/macOS
# 或
$env:RUST_LOG="info"  # PowerShell
# 或
set RUST_LOG=info     # CMD

# 运行服务器
cargo run -p ws_server
```

服务器将在127.0.0.2:8080上启动，并等待客户端连接。

### 运行客户端

在另一个终端中：

```bash
# 设置日志级别（可选）
export RUST_LOG=info  # Linux/macOS
# 或
$env:RUST_LOG="info"  # PowerShell
# 或
set RUST_LOG=info     # CMD

# 运行客户端
cargo run -p ws_client
```

客户端将连接到服务器，发送消息，然后关闭连接。

### 安全特性

- 服务器和客户端都使用TLS加密通信
- 双向证书认证确保双方身份的可信性
- 所有通信都经过加密，防止中间人攻击
- 证书验证确保只有受信任的客户端和服务器可以建立连接

## 注意事项

1. 如果你的系统没有127.0.0.2地址，你可能需要添加它：
   - Windows: `netsh interface ipv4 add address "Loopback" 127.0.0.2 255.0.0.0`
   - Linux: `sudo ip addr add 127.0.0.2/8 dev lo`

2. 确保在运行WebSocket服务器和客户端之前已经生成了所有必要的证书文件。

3. 如果想在不同的机器上运行客户端和服务器，需要修改IP地址配置和证书设置。

4. 项目使用Rust 2024 edition，请确保你的Rust工具链是最新的。

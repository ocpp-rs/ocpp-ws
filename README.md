cd# 证书验证与WebSocket安全通信工具

本项目提供了一套完整的证书生成、验证和基于WebSocket的安全通信解决方案，包含四个主要组件：

- `crate_cert`: 证书生成工具，用于创建CA证书及客户端/服务器证书
- `cert_check`: 证书验证工具，用于验证证书的有效性和信任链
- `ws_server`: 基于WebSocket的安全服务器，实现TLS加密和双向证书认证
- `ws_client`: 基于WebSocket的安全客户端，配合服务器实现安全通信

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

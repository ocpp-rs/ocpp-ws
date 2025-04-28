use std::fs::File;
use std::io::BufReader;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use log::{error, info};
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use rustls::ServerConfig;
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{WebSocketStream, accept_async};

// 加载证书和私钥
fn load_certs(path: &Path) -> Vec<CertificateDer<'static>> {
    let file = File::open(path).expect("无法打开证书文件");
    let mut reader = BufReader::new(file);
    rustls_pemfile::certs(&mut reader)
        .map(|result| result.expect("无法解析证书"))
        .collect()
}

// 加载私钥
fn load_private_key(path: &Path) -> PrivateKeyDer<'static> {
    let file = File::open(path).expect("无法打开私钥文件");
    let mut reader = BufReader::new(file);
    let mut keys = rustls_pemfile::pkcs8_private_keys(&mut reader)
        .collect::<Result<Vec<_>, _>>()
        .expect("无法解析私钥");
    if keys.is_empty() {
        panic!("没有找到私钥");
    }
    PrivateKeyDer::Pkcs8(keys.remove(0))
}

// 处理WebSocket连接
async fn handle_connection<S>(ws_stream: WebSocketStream<S>, addr: SocketAddr)
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    info!("新的WebSocket连接: {}", addr);

    // 将连接分为发送和接收部分
    let (mut ws_sender, mut ws_receiver) = ws_stream.split();

    // 处理接收到的消息
    while let Some(msg) = ws_receiver.next().await {
        match msg {
            Ok(msg) => {
                if msg.is_text() || msg.is_binary() {
                    info!("收到来自 {} 的消息: {:?}", addr, msg);

                    // 回复消息
                    let reply = format!("服务器收到: {}", msg);
                    if let Err(e) = ws_sender.send(Message::Text(reply)).await {
                        error!("发送消息错误: {}", e);
                        break;
                    }
                } else if msg.is_close() {
                    info!("客户端 {} 关闭连接", addr);
                    break;
                }
            }
            Err(e) => {
                error!("从 {} 接收消息错误: {}", addr, e);
                break;
            }
        }
    }

    info!("WebSocket连接关闭: {}", addr);
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化日志
    env_logger::init();

    // 服务器地址
    let addr = "127.0.0.1:9000";

    // 证书目录
    let cert_dir = Path::new("./crate_cert");

    // 加载服务器证书和私钥
    let server_cert = cert_dir.join("a_cert.pem");
    let server_key = cert_dir.join("a_key.pem");

    // 加载CA证书
    let ca_cert = cert_dir.join("ca_cert.pem");

    info!("服务器证书: {:?}", server_cert);
    info!("服务器私钥: {:?}", server_key);
    info!("CA证书: {:?}", ca_cert);

    info!("加载证书和私钥...");
    let certs = load_certs(&server_cert);
    let key = load_private_key(&server_key);
    let ca_certs = load_certs(&ca_cert);

    // 创建TLS配置
    let mut root_cert_store = rustls::RootCertStore::empty();
    for cert in ca_certs {
        root_cert_store.add(cert).expect("无法添加CA证书");
    }

    // 创建客户端证书验证器
    let client_verifier = rustls::server::WebPkiClientVerifier::builder(Arc::new(root_cert_store.clone()))
        .build()
        .expect("无法创建客户端证书验证器");

    // 创建服务器配置
    let server_config = ServerConfig::builder()
        .with_client_cert_verifier(client_verifier)
        .with_single_cert(certs, key)
        .expect("无法创建服务器配置");

    // 将配置转换为Arc
    let tls_config = Arc::new(server_config);

    // 创建TLS接受器
    let tls_acceptor = TlsAcceptor::from(tls_config);

    // 创建TCP监听器
    let listener = TcpListener::bind(addr).await?;
    info!("WebSocket服务器启动在: {}", addr);

    // 接受连接
    while let Ok((stream, addr)) = listener.accept().await {
        info!("接受TCP连接: {}", addr);

        // 克隆TLS接受器
        let acceptor = tls_acceptor.clone();

        // 处理连接
        tokio::spawn(async move {
            // 使用TLS接受器接受TLS连接
            info!("尝试接受TLS连接...");
            match acceptor.accept(stream).await {
                Ok(tls_stream) => {
                    info!("TLS连接成功，尝试接受WebSocket连接...");
                    // 接受WebSocket连接
                    match accept_async(tls_stream).await {
                        Ok(ws_stream) => {
                            info!("WebSocket连接成功");
                            handle_connection(ws_stream, addr).await;
                        }
                        Err(e) => {
                            error!("WebSocket握手失败: {}", e);
                        }
                    }
                }
                Err(e) => {
                    error!("TLS握手失败: {}", e);
                }
            }
        });
    }

    Ok(())
}

use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use log::{error, info};
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use rustls::{ClientConfig, RootCertStore};
use tokio::time::sleep;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{connect_async_tls_with_config, Connector};
use url::Url;

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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化日志
    env_logger::init();

    // 服务器地址
    let server_url = Url::parse("wss://127.0.0.1:9000")?;

    // 证书目录
    let cert_dir = Path::new("./crate_cert");

    // 加载客户端证书和私钥
    let client_cert = cert_dir.join("b_cert.pem");
    let client_key = cert_dir.join("b_key.pem");

    // 加载CA证书
    let ca_cert = cert_dir.join("ca_cert.pem");

    info!("客户端证书: {:?}", client_cert);
    info!("客户端私钥: {:?}", client_key);
    info!("CA证书: {:?}", ca_cert);

    info!("加载证书和私钥...");
    let client_certs = load_certs(&client_cert);
    let client_key = load_private_key(&client_key);
    let ca_certs = load_certs(&ca_cert);

    // 创建TLS配置
    let mut root_store = RootCertStore::empty();
    for cert in ca_certs {
        root_store.add(cert).expect("无法添加CA证书");
    }

    let client_config = ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_client_auth_cert(client_certs, client_key)
        .expect("无法创建客户端配置");

    // 创建TLS连接器
    let tls_config = Arc::new(client_config);
    let connector = Connector::Rustls(tls_config);

    // 连接到WebSocket服务器
    info!("连接到WebSocket服务器: {}", server_url);
    let (ws_stream, _) = connect_async_tls_with_config(server_url, None, false, Some(connector)).await?;
    info!("已连接到WebSocket服务器");

    // 将连接分为发送和接收部分
    let (mut ws_sender, mut ws_receiver) = ws_stream.split();

    // 发送消息
    let message = "你好，WebSocket服务器！";
    info!("发送消息: {}", message);
    ws_sender.send(Message::Text(message.to_string())).await?;

    // 接收消息
    if let Some(msg) = ws_receiver.next().await {
        match msg {
            Ok(msg) => {
                info!("收到服务器响应: {}", msg);
            }
            Err(e) => {
                error!("接收消息错误: {}", e);
            }
        }
    }

    // 发送更多消息
    for i in 1..=5 {
        let message = format!("消息 #{}", i);
        info!("发送消息: {}", message);
        ws_sender.send(Message::Text(message)).await?;

        // 等待响应
        if let Some(msg) = ws_receiver.next().await {
            match msg {
                Ok(msg) => {
                    info!("收到服务器响应: {}", msg);
                }
                Err(e) => {
                    error!("接收消息错误: {}", e);
                    break;
                }
            }
        }

        // 等待一秒
        sleep(Duration::from_secs(1)).await;
    }

    // 关闭连接
    info!("关闭WebSocket连接");
    ws_sender.close().await?;

    Ok(())
}

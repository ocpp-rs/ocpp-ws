use std::fs::File;
use std::io::BufReader;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use futures_util::{FutureExt, SinkExt, StreamExt};
use log::{debug, error, info, warn};
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio_rustls::TlsAcceptor;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::accept_async;

/// Message type for communication with WebSocket clients
#[derive(Debug, Clone)]
pub enum WsMessage {
    /// Text message
    Text(String),
    /// Binary message
    Binary(Vec<u8>),
    /// Close connection request
    Close,
}

impl From<Message> for WsMessage {
    fn from(msg: Message) -> Self {
        match msg {
            Message::Text(text) => WsMessage::Text(text),
            Message::Binary(data) => WsMessage::Binary(data),
            Message::Close(_) => WsMessage::Close,
            Message::Ping(_) | Message::Pong(_) | Message::Frame(_) => {
                // These are handled internally by the WebSocket implementation
                WsMessage::Text("".to_string())
            }
        }
    }
}

impl From<WsMessage> for Message {
    fn from(msg: WsMessage) -> Self {
        match msg {
            WsMessage::Text(text) => Message::Text(text),
            WsMessage::Binary(data) => Message::Binary(data),
            WsMessage::Close => Message::Close(None),
        }
    }
}

/// Client identifier for WebSocket connections
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ClientId(pub String);

/// Configuration for WebSocket server
#[derive(Debug, Clone)]
pub struct WsServerConfig {
    /// Address to bind the server to
    pub addr: String,
    /// Path to server certificate file
    pub cert_path: PathBuf,
    /// Path to server private key file
    pub key_path: PathBuf,
    /// Path to CA certificate file
    pub ca_cert_path: PathBuf,
    /// Maximum number of concurrent connections
    pub max_connections: usize,
    /// Connection timeout in seconds
    pub connection_timeout: u64,
    /// Enable client certificate verification
    pub client_cert_required: bool,
}

impl Default for WsServerConfig {
    fn default() -> Self {
        Self {
            addr: "127.0.0.1:9000".to_string(),
            cert_path: PathBuf::from("./crate_cert/a_cert.pem"),
            key_path: PathBuf::from("./crate_cert/a_key.pem"),
            ca_cert_path: PathBuf::from("./crate_cert/ca_cert.pem"),
            max_connections: 1000,
            connection_timeout: 30,
            client_cert_required: true,
        }
    }
}

/// WebSocket Server handler for processing incoming messages
///
/// Implement this trait to handle incoming WebSocket messages.
pub trait ServerHandler: Send + Sync + 'static {
    /// Called when a client connects
    fn on_connect(&self, client_id: ClientId, addr: SocketAddr);
    
    /// Called when a client disconnects
    fn on_disconnect(&self, client_id: ClientId);
    
    /// Called when a message is received from a client
    ///
    /// Return optional response message
    fn on_message(&self, client_id: ClientId, message: WsMessage) -> Option<WsMessage>;
    
    /// Called when an error occurs
    fn on_error(&self, client_id: Option<ClientId>, error: String);
}

/// Client connection manager
struct ClientConnection {
    client_id: ClientId,
    tx: mpsc::Sender<WsMessage>,
}

/// WebSocket Server
pub struct WsServer {
    config: WsServerConfig,
    handler: Arc<dyn ServerHandler>,
    tls_acceptor: TlsAcceptor,
    clients: Arc<tokio::sync::Mutex<Vec<ClientConnection>>>,
}

impl WsServer {
    /// Create a new WebSocket server with the provided configuration and handler
    ///
    /// # Arguments
    ///
    /// * `config` - Server configuration
    /// * `handler` - Server handler for processing messages
    ///
    /// # Returns
    ///
    /// * `Result<Self, String>` - New server instance or error
    pub fn new(config: WsServerConfig, handler: impl ServerHandler) -> Result<Self, String> {
        let tls_acceptor = Self::create_tls_acceptor(&config)
            .map_err(|e| format!("Failed to create TLS acceptor: {}", e))?;
            
        Ok(Self {
            config,
            handler: Arc::new(handler),
            tls_acceptor,
            clients: Arc::new(tokio::sync::Mutex::new(Vec::new())),
        })
    }
    
    /// Start the WebSocket server
    ///
    /// # Returns
    ///
    /// * `Result<(), String>` - Success or error
    pub async fn start(&self) -> Result<(), String> {
        let listener = TcpListener::bind(&self.config.addr)
            .await
            .map_err(|e| format!("Failed to bind to address {}: {}", self.config.addr, e))?;
            
        info!("WebSocket server started on {}", self.config.addr);
        
        loop {
            match listener.accept().await {
                Ok((stream, addr)) => {
                    debug!("New TCP connection from: {}", addr);
                    
                    // Clone necessary references for the task
                    let acceptor = self.tls_acceptor.clone();
                    let handler = self.handler.clone();
                    let clients = self.clients.clone();
                    let connection_timeout = Duration::from_secs(self.config.connection_timeout);
                    
                    // Generate a unique client ID
                    let client_id = ClientId(format!("client-{}", uuid_simple()));
                    let client_id_clone = client_id.clone();
                    
                    tokio::spawn(async move {
                        if let Err(e) = Self::handle_connection(
                            stream, 
                            addr, 
                            acceptor, 
                            handler, 
                            clients, 
                            client_id_clone, 
                            connection_timeout
                        ).await {
                            error!("Connection error for {}: {}", addr, e);
                        }
                    });
                }
                Err(e) => {
                    error!("Failed to accept connection: {}", e);
                }
            }
            
            // Check if we've reached the maximum number of connections
            let client_count = self.clients.lock().await.len();
            if client_count >= self.config.max_connections {
                warn!("Maximum connections reached: {}", client_count);
                
                // Small delay to avoid CPU spinning
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
    }
    
    /// Broadcast a message to all connected clients
    ///
    /// # Arguments
    ///
    /// * `message` - Message to broadcast
    ///
    /// # Returns
    ///
    /// * `Result<usize, String>` - Number of clients that received the message or error
    pub async fn broadcast(&self, message: WsMessage) -> Result<usize, String> {
        let clients = self.clients.lock().await;
        let mut sent_count = 0;
        
        for client in clients.iter() {
            if client.tx.send(message.clone()).await.is_ok() {
                sent_count += 1;
            }
        }
        
        Ok(sent_count)
    }
    
    /// Send a message to a specific client
    ///
    /// # Arguments
    ///
    /// * `client_id` - ID of the client to send the message to
    /// * `message` - Message to send
    ///
    /// # Returns
    ///
    /// * `Result<(), String>` - Success or error
    pub async fn send_to_client(&self, client_id: &ClientId, message: WsMessage) -> Result<(), String> {
        let clients = self.clients.lock().await;
        
        for client in clients.iter() {
            if client.client_id == *client_id {
                return client.tx.send(message)
                    .await
                    .map_err(|_| format!("Failed to send message to client {}", client_id.0));
            }
        }
        
        Err(format!("Client not found: {}", client_id.0))
    }
    
    /// Get the number of connected clients
    ///
    /// # Returns
    ///
    /// * `usize` - Number of connected clients
    pub async fn client_count(&self) -> usize {
        self.clients.lock().await.len()
    }
    
    /// Get a list of connected client IDs
    ///
    /// # Returns
    ///
    /// * `Vec<ClientId>` - List of connected client IDs
    pub async fn client_list(&self) -> Vec<ClientId> {
        let clients = self.clients.lock().await;
        clients.iter().map(|c| c.client_id.clone()).collect()
    }
    
    /// Handle a new WebSocket connection
    async fn handle_connection(
        stream: TcpStream,
        addr: SocketAddr,
        acceptor: TlsAcceptor,
        handler: Arc<dyn ServerHandler>,
        clients: Arc<tokio::sync::Mutex<Vec<ClientConnection>>>,
        client_id: ClientId,
        connection_timeout: Duration,
    ) -> Result<(), String> {
        // Apply timeout to the TLS handshake
        let tls_handshake = tokio::time::timeout(
            connection_timeout,
            acceptor.accept(stream),
        ).await
            .map_err(|_| format!("TLS handshake timed out after {} seconds", connection_timeout.as_secs()))?
            .map_err(|e| format!("TLS handshake failed: {}", e))?;
            
        debug!("TLS handshake successful for {}", addr);
        
        // Apply timeout to the WebSocket handshake
        let ws_stream = tokio::time::timeout(
            connection_timeout,
            accept_async(tls_handshake),
        ).await
            .map_err(|_| format!("WebSocket handshake timed out after {} seconds", connection_timeout.as_secs()))?
            .map_err(|e| format!("WebSocket handshake failed: {}", e))?;
            
        debug!("WebSocket handshake successful for {}", addr);
        
        // Create message channels
        let (tx, mut rx) = mpsc::channel::<WsMessage>(100);
        
        // Register the client
        {
            let mut clients_lock = clients.lock().await;
            clients_lock.push(ClientConnection {
                client_id: client_id.clone(),
                tx: tx.clone(),
            });
            
            info!("Client connected: {} from {}", client_id.0, addr);
        }
        
        // Notify the handler about the new connection
        handler.on_connect(client_id.clone(), addr);
        
        // Split WebSocket stream
        let (ws_sender, ws_receiver) = ws_stream.split();
        
        // Forward outgoing messages to the WebSocket
        let mut send_task = {
            let mut ws_sender = ws_sender;
            let client_id_for_send = client_id.clone();
            let handler_for_send = handler.clone();
            
            async move {
                while let Some(msg) = rx.recv().await {
                    match ws_sender.send(msg.into()).await {
                        Ok(_) => {
                            debug!("Message sent to client {}", client_id_for_send.0);
                        }
                        Err(e) => {
                            let error_msg = format!("Failed to send message: {}", e);
                            handler_for_send.on_error(Some(client_id_for_send.clone()), error_msg);
                            break;
                        }
                    }
                }
                
                // Try to close the connection gracefully
                let _ = ws_sender.close().await;
                
                debug!("Send task completed for client {}", client_id_for_send.0);
            }.boxed()
        };
        
        // Process incoming messages from the WebSocket
        let mut receive_task = {
            let mut ws_receiver = ws_receiver;
            let handler_for_recv = handler.clone();
            let client_id_for_recv = client_id.clone();
            let tx_for_recv = tx.clone();
            
            async move {
                while let Some(result) = ws_receiver.next().await {
                    match result {
                        Ok(msg) => {
                            if msg.is_close() {
                                debug!("Client {} requested close", client_id_for_recv.0);
                                break;
                            }
                            
                            // Convert to our message type
                            let ws_msg = WsMessage::from(msg);
                            
                            // Let the handler process the message
                            if let Some(response) = handler_for_recv.on_message(client_id_for_recv.clone(), ws_msg) {
                                // Send the response if provided
                                if tx_for_recv.send(response).await.is_err() {
                                    break;
                                }
                            }
                        }
                        Err(e) => {
                            let error_msg = format!("Error receiving message: {}", e);
                            handler_for_recv.on_error(Some(client_id_for_recv.clone()), error_msg);
                            break;
                        }
                    }
                }
                
                debug!("Receive task completed for client {}", client_id_for_recv.0);
            }.boxed()
        };
        
        // Wait for either task to complete
        tokio::select! {
            _ = &mut send_task => {},
            _ = &mut receive_task => {},
        }
        
        // Clean up
        Self::remove_client(clients, client_id.clone()).await;
        handler.on_disconnect(client_id.clone());
        
        info!("Client disconnected: {} from {}", client_id.0, addr);
        Ok(())
    }
    
    /// Remove a client from the clients list
    async fn remove_client(
        clients: Arc<tokio::sync::Mutex<Vec<ClientConnection>>>,
        client_id: ClientId,
    ) {
        let mut clients_lock = clients.lock().await;
        if let Some(pos) = clients_lock.iter().position(|c| c.client_id == client_id) {
            clients_lock.remove(pos);
        }
    }
    
    /// Create a TLS acceptor from the server configuration
    fn create_tls_acceptor(config: &WsServerConfig) -> Result<TlsAcceptor, Box<dyn std::error::Error>> {
        // Load certificates and keys
        info!("Loading certificates and keys...");
        let certs = load_certs(&config.cert_path)?;
        let key = load_private_key(&config.key_path)?;
        let ca_certs = load_certs(&config.ca_cert_path)?;
        
        // Create root certificate store
        let mut root_cert_store = rustls::RootCertStore::empty();
        for cert in ca_certs {
            root_cert_store.add(cert)?;
        }
        
        // Create a server config builder
        let server_config = if config.client_cert_required {
            // Create client certificate verifier
            let client_verifier = rustls::server::WebPkiClientVerifier::builder(Arc::new(root_cert_store))
                .build()?;
                
            // Configure TLS server with client authentication
            rustls::ServerConfig::builder()
                .with_client_cert_verifier(client_verifier)
                .with_single_cert(certs, key)?
        } else {
            // Configure TLS server without client authentication
            rustls::ServerConfig::builder()
                .with_no_client_auth()
                .with_single_cert(certs, key)?
        };
            
        // Create TLS acceptor
        Ok(TlsAcceptor::from(Arc::new(server_config)))
    }
}

/// Load certificates from a file
///
/// # Arguments
///
/// * `path` - Path to the certificate file
///
/// # Returns
///
/// * `Result<Vec<CertificateDer<'static>>, Box<dyn std::error::Error>>` - Loaded certificates or error
fn load_certs(path: &Path) -> Result<Vec<CertificateDer<'static>>, Box<dyn std::error::Error>> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    let mut certs = Vec::new();
    
    for cert_result in rustls_pemfile::certs(&mut reader) {
        let cert = cert_result?;
        certs.push(cert);
    }
    
    if certs.is_empty() {
        return Err(format!("No certificates found in {}", path.display()).into());
    }
    
    Ok(certs)
}

/// Load private key from a file
///
/// # Arguments
///
/// * `path` - Path to the private key file
///
/// # Returns
///
/// * `Result<PrivateKeyDer<'static>, Box<dyn std::error::Error>>` - Loaded private key or error
fn load_private_key(path: &Path) -> Result<PrivateKeyDer<'static>, Box<dyn std::error::Error>> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    
    // Try PKCS8 format first
    let mut pkcs8_keys = Vec::new();
    for key_result in rustls_pemfile::pkcs8_private_keys(&mut reader) {
        pkcs8_keys.push(key_result?);
    }
    
    if !pkcs8_keys.is_empty() {
        return Ok(PrivateKeyDer::Pkcs8(pkcs8_keys.remove(0)));
    }
    
    // Reset reader position
    reader = BufReader::new(File::open(path)?);
    
    // Try RSA format
    let mut rsa_keys = Vec::new();
    for key_result in rustls_pemfile::rsa_private_keys(&mut reader) {
        rsa_keys.push(key_result?);
    }
    
    if !rsa_keys.is_empty() {
        return Ok(PrivateKeyDer::Pkcs1(rsa_keys.remove(0)));
    }
    
    // Reset reader position
    reader = BufReader::new(File::open(path)?);
    
    // Try EC format
    let mut ec_keys = Vec::new();
    for key_result in rustls_pemfile::ec_private_keys(&mut reader) {
        ec_keys.push(key_result?);
    }
    
    if !ec_keys.is_empty() {
        return Ok(PrivateKeyDer::Sec1(ec_keys.remove(0)));
    }
    
    Err(format!("No private keys found in {}", path.display()).into())
}

/// Generate a simple UUID-like string for client IDs
fn uuid_simple() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
        
    format!(
        "{:x}{:x}",
        now.as_secs(),
        now.subsec_nanos()
    )
}
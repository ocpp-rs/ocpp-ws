use std::fs::File;
use std::io::BufReader;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use std::collections::HashMap;

use futures_util::{SinkExt, StreamExt};
use log::{debug, info};
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, RwLock};
use tokio_rustls::TlsAcceptor;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::accept_hdr_async;
use tokio_tungstenite::tungstenite::handshake::server::{Request, Response, ErrorResponse};
use crate::path_utils::PathInfo;

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

/// Connection interface for bidirectional WebSocket communication
///
/// This interface provides methods for sending and receiving messages
/// with a specific WebSocket client, along with access to connection metadata.
#[derive(Clone)]
pub struct ConnectionInterface {
    client_id: ClientId,
    addr: SocketAddr,
    path_info: PathInfo,
    /// Selected WebSocket subprotocol (e.g., "ocpp2.1")
    subprotocol: Option<String>,
    /// Channel to send messages to this client
    tx: mpsc::Sender<WsMessage>,
    /// Channel to receive messages from this client
    rx: Arc<tokio::sync::Mutex<mpsc::Receiver<WsMessage>>>,
}

impl ConnectionInterface {
    /// Get the client ID
    pub fn client_id(&self) -> &ClientId {
        &self.client_id
    }

    /// Get the client's socket address
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// Get the client's path information
    pub fn path_info(&self) -> &PathInfo {
        &self.path_info
    }

    /// Get the URL suffix for this connection
    ///
    /// Returns the URL path suffix that was used when the client connected.
    /// For example, if a client connected to `wss://127.0.0.1:9999/ocpp/CS001`,
    /// this method will return `"ocpp/CS001"`.
    pub fn url_suffix(&self) -> &str {
        self.path_info.suffix()
    }

    /// Get the decoded URL suffix for this connection
    ///
    /// Returns the URL-decoded path suffix. URL encoding like `%7C` will be
    /// converted to `|`, `%20` to space, etc.
    pub fn url_suffix_decoded(&self) -> &str {
        self.path_info.decoded_suffix()
    }

    /// Get the selected WebSocket subprotocol for this connection
    ///
    /// Returns the subprotocol that was negotiated during the WebSocket handshake.
    /// For OCPP connections, this might be "ocpp2.1", "ocpp2.0.1", or "ocpp1.6".
    pub fn subprotocol(&self) -> Option<&str> {
        self.subprotocol.as_deref()
    }

    /// Send a message to this client
    ///
    /// # Arguments
    /// * `message` - Message to send
    ///
    /// # Returns
    /// * `Result<(), String>` - Success or error
    pub async fn send_message(&self, message: WsMessage) -> Result<(), String> {
        self.tx.send(message)
            .await
            .map_err(|_| format!("Failed to send message to client {}", self.client_id.0))
    }

    /// Read a message from this client (non-blocking)
    ///
    /// This method attempts to read a message that was sent by the client.
    /// It returns immediately with None if no message is available.
    ///
    /// # Returns
    /// * `Option<WsMessage>` - The message if available, None if no message is waiting
    pub async fn try_receive_message(&self) -> Option<WsMessage> {
        let mut rx = self.rx.lock().await;
        rx.try_recv().ok()
    }

    /// Read a message from this client (blocking)
    ///
    /// This method waits for a message from the client. It will block until
    /// a message is received or the connection is closed.
    ///
    /// # Returns
    /// * `Option<WsMessage>` - The message if received, None if connection is closed
    pub async fn receive_message(&self) -> Option<WsMessage> {
        let mut rx = self.rx.lock().await;
        rx.recv().await
    }

    /// Check if this client is still connected
    ///
    /// This method checks if the message channel is still open, which indicates
    /// that the client connection is still active.
    ///
    /// # Returns
    /// * `bool` - True if the client is still connected, false otherwise
    pub fn is_connected(&self) -> bool {
        !self.tx.is_closed()
    }
}

/// Manager for connection interfaces
///
/// This structure manages active connection interfaces and provides thread-safe access
/// to them. It automatically cleans up interfaces when clients disconnect.
#[derive(Clone)]
struct ConnectionManager {
    interfaces: Arc<RwLock<HashMap<ClientId, ConnectionInterface>>>,
}

impl ConnectionManager {
    /// Create a new connection manager
    fn new() -> Self {
        Self {
            interfaces: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Add a new connection interface
    async fn add_interface(&self, client_id: ClientId, interface: ConnectionInterface) {
        let mut interfaces = self.interfaces.write().await;
        interfaces.insert(client_id, interface);
    }

    /// Get a connection interface by ID
    async fn get_interface(&self, client_id: &ClientId) -> Option<ConnectionInterface> {
        let interfaces = self.interfaces.read().await;
        interfaces.get(client_id).cloned()
    }

    /// Get all active connection interfaces
    async fn get_all_interfaces(&self) -> Vec<ConnectionInterface> {
        let interfaces = self.interfaces.read().await;
        interfaces.values().cloned().collect()
    }

    /// Get the number of active interfaces
    async fn interface_count(&self) -> usize {
        let interfaces = self.interfaces.read().await;
        interfaces.len()
    }

    /// Get all client IDs with active interfaces
    async fn get_client_ids(&self) -> Vec<ClientId> {
        let interfaces = self.interfaces.read().await;
        interfaces.keys().cloned().collect()
    }

    /// Remove a connection interface when client disconnects
    async fn remove_interface(&self, client_id: &ClientId) -> Option<ConnectionInterface> {
        let mut interfaces = self.interfaces.write().await;
        interfaces.remove(client_id)
    }
}

/// WebSocket Server
///
/// This server provides connection interfaces for bidirectional communication
/// instead of running background threads. Each connection provides methods
/// for sending and receiving messages along with access to connection metadata.
pub struct WsServer {
    config: WsServerConfig,
    tls_acceptor: TlsAcceptor,
    listener: Option<TcpListener>,
    connection_manager: ConnectionManager,
}

impl WsServer {
    /// Create a new WebSocket server with the provided configuration
    ///
    /// # Arguments
    ///
    /// * `config` - Server configuration
    ///
    /// # Returns
    ///
    /// * `Result<Self, String>` - New server instance or error
    pub fn new(config: WsServerConfig) -> Result<Self, String> {
        let tls_acceptor = Self::create_tls_acceptor(&config)
            .map_err(|e| format!("Failed to create TLS acceptor: {}", e))?;

        Ok(Self {
            config,
            tls_acceptor,
            listener: None,
            connection_manager: ConnectionManager::new(),
        })
    }

    /// Initialize the server and bind to the configured address
    ///
    /// This method prepares the server for accepting connections but does not
    /// start accepting them yet. Call `accept_connection()` to handle incoming connections.
    ///
    /// # Returns
    ///
    /// * `Result<(), String>` - Success or error
    pub async fn initialize(&mut self) -> Result<(), String> {
        let listener = TcpListener::bind(&self.config.addr)
            .await
            .map_err(|e| format!("Failed to bind to address {}: {}", self.config.addr, e))?;

        info!("WebSocket server initialized on {}", self.config.addr);
        self.listener = Some(listener);
        Ok(())
    }
    
    /// Accept a new WebSocket connection
    ///
    /// This method waits for and accepts a single incoming connection, returning
    /// a `ConnectionInterface` that can be used for bidirectional communication.
    /// The server must be initialized first using `initialize()`.
    ///
    /// # Returns
    ///
    /// * `Result<ConnectionInterface, String>` - Connection interface or error
    pub async fn accept_connection(&self) -> Result<ConnectionInterface, String> {
        let listener = self.listener.as_ref()
            .ok_or_else(|| "Server not initialized. Call initialize() first.".to_string())?;

        // Accept a new connection
        let (stream, addr) = listener.accept().await
            .map_err(|e| format!("Failed to accept connection: {}", e))?;

        debug!("New TCP connection from: {}", addr);

        // Generate a unique client ID
        let client_id = ClientId(format!("client-{}", uuid_simple()));
        let connection_timeout = Duration::from_secs(self.config.connection_timeout);

        // Handle the connection and create interface
        self.create_connection_interface(stream, addr, client_id, connection_timeout).await
    }


    
    /// Get a connection interface for a specific client
    ///
    /// # Arguments
    ///
    /// * `client_id` - The ID of the client to get an interface for
    ///
    /// # Returns
    ///
    /// * `Option<ConnectionInterface>` - The connection interface if the client is connected, None otherwise
    pub async fn get_connection_interface(&self, client_id: &ClientId) -> Option<ConnectionInterface> {
        self.connection_manager.get_interface(client_id).await
    }

    /// Get all active connection interfaces
    ///
    /// # Returns
    ///
    /// * `Vec<ConnectionInterface>` - List of all active connection interfaces
    pub async fn get_all_connection_interfaces(&self) -> Vec<ConnectionInterface> {
        self.connection_manager.get_all_interfaces().await
    }

    /// Get the number of active connections
    ///
    /// # Returns
    ///
    /// * `usize` - Number of active connections
    pub async fn connection_count(&self) -> usize {
        self.connection_manager.interface_count().await
    }

    /// Get all client IDs with active connections
    ///
    /// # Returns
    ///
    /// * `Vec<ClientId>` - List of client IDs with active connections
    pub async fn get_connected_client_ids(&self) -> Vec<ClientId> {
        self.connection_manager.get_client_ids().await
    }

    /// Check if a specific client is still connected
    ///
    /// # Arguments
    ///
    /// * `client_id` - The ID of the client to check
    ///
    /// # Returns
    ///
    /// * `bool` - True if the client is still connected, false otherwise
    pub async fn is_client_connected(&self, client_id: &ClientId) -> bool {
        self.connection_manager.get_interface(client_id).await.is_some()
    }





    /// Create a connection interface from a TCP stream
    ///
    /// This method handles the TLS and WebSocket handshakes and creates a
    /// `ConnectionInterface` for bidirectional communication.
    ///
    /// # Arguments
    ///
    /// * `stream` - TCP stream from the client
    /// * `addr` - Client socket address
    /// * `client_id` - Unique client identifier
    /// * `connection_timeout` - Timeout for handshakes
    ///
    /// # Returns
    ///
    /// * `Result<ConnectionInterface, String>` - Connection interface or error
    async fn create_connection_interface(
        &self,
        stream: TcpStream,
        addr: SocketAddr,
        client_id: ClientId,
        connection_timeout: Duration,
    ) -> Result<ConnectionInterface, String> {
        // Apply timeout to the TLS handshake
        let tls_handshake = tokio::time::timeout(
            connection_timeout,
            self.tls_acceptor.accept(stream),
        ).await
            .map_err(|_| format!("TLS handshake timed out after {} seconds", connection_timeout.as_secs()))?
            .map_err(|e| format!("TLS handshake failed: {}", e))?;

        debug!("TLS handshake successful for {}", addr);

        // Apply timeout to the WebSocket handshake with header callback to extract path and handle subprotocols
        use std::sync::{Arc, OnceLock};
        let connection_path: Arc<OnceLock<String>> = Arc::new(OnceLock::new());
        let selected_subprotocol: Arc<OnceLock<Option<String>>> = Arc::new(OnceLock::new());
        let path_clone = connection_path.clone();
        let subprotocol_clone = selected_subprotocol.clone();

        let callback = move |request: &Request, mut response: Response| -> Result<Response, ErrorResponse> {
            // Extract the path from the request URI
            let path = request.uri().path().to_string();
            let _ = path_clone.set(path.clone());
            debug!("WebSocket connection request path: {}", path);

            // Handle subprotocol negotiation
            if let Some(protocols_header) = request.headers().get("sec-websocket-protocol") {
                if let Ok(protocols_str) = protocols_header.to_str() {
                    debug!("Client requested subprotocols: {}", protocols_str);

                    // Parse requested protocols
                    let requested_protocols: Vec<&str> = protocols_str
                        .split(',')
                        .map(|s| s.trim())
                        .collect();

                    // OCPP protocol priority: prefer newer versions
                    let supported_protocols = ["ocpp2.1", "ocpp2.0.1", "ocpp1.6"];
                    let mut selected_protocol = None;

                    // Find the first supported protocol in priority order
                    for supported in &supported_protocols {
                        if requested_protocols.contains(supported) {
                            selected_protocol = Some(supported.to_string());
                            break;
                        }
                    }

                    if let Some(protocol) = &selected_protocol {
                        info!("Selected subprotocol: {}", protocol);
                        response.headers_mut().insert(
                            "sec-websocket-protocol",
                            protocol.parse().unwrap()
                        );
                    } else {
                        debug!("No supported subprotocol found in: {:?}", requested_protocols);
                    }

                    let _ = subprotocol_clone.set(selected_protocol);
                } else {
                    debug!("Invalid subprotocol header format");
                    let _ = subprotocol_clone.set(None);
                }
            } else {
                debug!("No subprotocol header found");
                let _ = subprotocol_clone.set(None);
            }

            Ok(response)
        };

        let ws_stream = tokio::time::timeout(
            connection_timeout,
            accept_hdr_async(tls_handshake, callback),
        ).await
            .map_err(|_| format!("WebSocket handshake timed out after {} seconds", connection_timeout.as_secs()))?
            .map_err(|e| format!("WebSocket handshake failed: {}", e))?;

        let extracted_path = connection_path.get()
            .cloned()
            .unwrap_or_else(|| "/".to_string());
        let negotiated_subprotocol = selected_subprotocol.get()
            .cloned()
            .unwrap_or(None);

        debug!("WebSocket handshake successful for {} with path: {}", addr, extracted_path);
        if let Some(ref protocol) = negotiated_subprotocol {
            info!("Subprotocol negotiated: {}", protocol);
        }

        // Create PathInfo for enhanced path processing
        let path_info = PathInfo::new(extracted_path);
        debug!("Path info created: raw='{}', decoded='{}', suffix='{}'",
               path_info.raw(), path_info.decoded(), path_info.decoded_suffix());

        // Create message channels
        let (tx, rx) = mpsc::channel::<WsMessage>(100);
        let (app_tx, app_rx) = mpsc::channel::<WsMessage>(100);

        // Create the connection interface
        let interface = ConnectionInterface {
            client_id: client_id.clone(),
            addr,
            path_info: path_info.clone(),
            subprotocol: negotiated_subprotocol,
            tx: tx.clone(),
            rx: Arc::new(tokio::sync::Mutex::new(app_rx)),
        };

        // Register the interface
        self.connection_manager.add_interface(client_id.clone(), interface.clone()).await;

        info!("Client connected: {} from {} with path: {}", client_id.0, addr, path_info.raw());

        // Split WebSocket stream and start message handling tasks
        let (ws_sender, ws_receiver) = ws_stream.split();

        // Start background tasks for message handling
        self.start_message_tasks(ws_sender, ws_receiver, rx, app_tx, client_id.clone()).await;

        Ok(interface)
    }

    /// Start background tasks for handling WebSocket messages
    ///
    /// This method starts two background tasks:
    /// 1. Send task: forwards outgoing messages to the WebSocket
    /// 2. Receive task: processes incoming messages from the WebSocket
    async fn start_message_tasks(
        &self,
        mut ws_sender: futures_util::stream::SplitSink<tokio_tungstenite::WebSocketStream<tokio_rustls::server::TlsStream<TcpStream>>, Message>,
        mut ws_receiver: futures_util::stream::SplitStream<tokio_tungstenite::WebSocketStream<tokio_rustls::server::TlsStream<TcpStream>>>,
        mut rx: mpsc::Receiver<WsMessage>,
        app_tx: mpsc::Sender<WsMessage>,
        client_id: ClientId,
    ) {
        let client_id_for_cleanup = client_id.clone();

        // Forward outgoing messages to the WebSocket
        let send_task = {
            let client_id_for_send = client_id.clone();
            let connection_manager_for_send = self.connection_manager.clone();

            async move {
                let mut connection_broken = false;
                while let Some(msg) = rx.recv().await {
                    match ws_sender.send(msg.into()).await {
                        Ok(_) => {
                            debug!("Message sent to client {}", client_id_for_send.0);
                        }
                        Err(e) => {
                            debug!("Failed to send message to client {}: {}", client_id_for_send.0, e);
                            connection_broken = true;
                            break;
                        }
                    }
                }

                // Try to close the connection gracefully
                let _ = ws_sender.close().await;
                debug!("Send task completed for client {}", client_id_for_send.0);

                // If connection was broken during send, clean up immediately
                if connection_broken {
                    if let Some(removed_interface) = connection_manager_for_send.remove_interface(&client_id_for_send).await {
                        info!("Client disconnected (send failed): {} from {}",
                              removed_interface.client_id().0,
                              removed_interface.addr());
                    }
                }
            }
        };

        // Process incoming messages from the WebSocket
        let receive_task = {
            let client_id_for_recv = client_id.clone();
            let app_tx_for_recv = app_tx.clone();

            async move {
                let mut disconnect_reason = "unknown";

                while let Some(result) = ws_receiver.next().await {
                    match result {
                        Ok(msg) => {
                            if msg.is_close() {
                                debug!("Client {} requested close", client_id_for_recv.0);
                                disconnect_reason = "client_requested_close";
                                break;
                            }

                            // Convert to our message type
                            let ws_msg = WsMessage::from(msg);

                            // Forward message to application layer (non-blocking)
                            if app_tx_for_recv.try_send(ws_msg).is_err() {
                                debug!("Application layer not reading messages for client {}", client_id_for_recv.0);
                                // Continue anyway, don't disconnect just because app isn't reading
                            }
                        }
                        Err(e) => {
                            debug!("Error receiving message from client {}: {}", client_id_for_recv.0, e);
                            disconnect_reason = "connection_error";
                            break;
                        }
                    }
                }

                debug!("Receive task completed for client {} (reason: {})", client_id_for_recv.0, disconnect_reason);
            }
        };

        // Spawn both tasks
        tokio::spawn(send_task);

        // Spawn receive task with cleanup
        let connection_manager_clone = self.connection_manager.clone();
        tokio::spawn(async move {
            receive_task.await;

            // Clean up the connection interface when the receive task completes
            if let Some(removed_interface) = connection_manager_clone.remove_interface(&client_id_for_cleanup).await {
                info!("Client disconnected: {} from {}",
                      removed_interface.client_id().0,
                      removed_interface.addr());
            }
        });
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
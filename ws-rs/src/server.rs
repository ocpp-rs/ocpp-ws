use std::fs::File;
use std::io::BufReader;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use std::collections::HashMap;

use futures_util::{FutureExt, SinkExt, StreamExt};
use log::{debug, error, info, warn};
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

/// WebSocket Server handler for processing incoming messages
///
/// Implement this trait to handle incoming WebSocket messages.
pub trait ServerHandler: Send + Sync + 'static {
    /// Called when a client connects
    ///
    /// # Arguments
    /// * `client_id` - Unique identifier for the client
    /// * `addr` - Socket address of the client
    /// * `path` - URL path from the WebSocket connection request (e.g., "/ocpp/CS001")
    fn on_connect(&self, client_id: ClientId, addr: SocketAddr, path: String);

    /// Called when a client connects with enhanced path information
    ///
    /// This method provides additional URL path processing capabilities including
    /// automatic URL decoding and path analysis. If not implemented, it will
    /// fall back to calling `on_connect` with the raw path.
    ///
    /// # Arguments
    /// * `client_id` - Unique identifier for the client
    /// * `addr` - Socket address of the client
    /// * `path_info` - Processed path information with URL decoding and analysis
    fn on_connect_with_path(&self, client_id: ClientId, addr: SocketAddr, path_info: PathInfo) {
        // Default implementation falls back to the original method
        self.on_connect(client_id, addr, path_info.raw_path);
    }

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

/// Handle for interacting with a specific client connection
///
/// This handle provides methods for bidirectional communication with a client,
/// including sending messages and managing client-specific data storage.
/// The handle is thread-safe and can be shared across multiple tasks.
///
/// # Example
///
/// ```ignore
/// // Get a handle for a specific client
/// if let Some(handle) = server.get_client_handle(&client_id).await {
///     // Send a message to the client
///     handle.send_message(WsMessage::Text("Hello!".to_string())).await?;
///
///     // Store some data for this client
///     handle.write_data("session_id".to_string(), "abc123".as_bytes().to_vec()).await?;
///
///     // Read data back
///     if let Some(data) = handle.read_data("session_id").await? {
///         let session_id = String::from_utf8(data)?;
///         println!("Session ID: {}", session_id);
///     }
/// }
/// ```
#[derive(Clone)]
pub struct ClientHandle {
    client_id: ClientId,
    addr: SocketAddr,
    path_info: PathInfo,
    /// Channel to send messages to this client
    tx: mpsc::Sender<WsMessage>,
    /// Channel to receive messages from this client (for application use)
    message_rx: Arc<tokio::sync::Mutex<mpsc::Receiver<WsMessage>>>,
}

impl ClientHandle {
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

    /// Send a message to this client
    ///
    /// # Arguments
    ///
    /// * `message` - Message to send
    ///
    /// # Returns
    ///
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
    ///
    /// * `Option<WsMessage>` - The message if available, None if no message is waiting
    pub async fn try_read_message(&self) -> Option<WsMessage> {
        let mut rx = self.message_rx.lock().await;
        rx.try_recv().ok()
    }

    /// Read a message from this client (blocking)
    ///
    /// This method waits for a message from the client. It will block until
    /// a message is received or the connection is closed.
    ///
    /// # Returns
    ///
    /// * `Option<WsMessage>` - The message if received, None if connection is closed
    pub async fn read_message(&self) -> Option<WsMessage> {
        let mut rx = self.message_rx.lock().await;
        rx.recv().await
    }



    /// Check if this client is still connected
    ///
    /// This method checks if the message channel is still open, which indicates
    /// that the client connection is still active.
    ///
    /// # Returns
    ///
    /// * `bool` - True if the client is still connected, false otherwise
    pub async fn is_connected(&self) -> bool {
        !self.tx.is_closed()
    }
}

/// Manager for client handles
///
/// This structure manages active client handles and provides thread-safe access
/// to them. It automatically cleans up handles when clients disconnect.
struct ClientHandleManager {
    handles: Arc<RwLock<HashMap<ClientId, ClientHandle>>>,
}

impl ClientHandleManager {
    /// Create a new client handle manager
    fn new() -> Self {
        Self {
            handles: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get a client handle by ID
    async fn get_handle(&self, client_id: &ClientId) -> Option<ClientHandle> {
        let handles = self.handles.read().await;
        handles.get(client_id).cloned()
    }

    /// Get all active client handles
    async fn get_all_handles(&self) -> Vec<ClientHandle> {
        let handles = self.handles.read().await;
        handles.values().cloned().collect()
    }

    /// Get the number of active handles
    async fn handle_count(&self) -> usize {
        let handles = self.handles.read().await;
        handles.len()
    }

    /// Get all client IDs with active handles
    async fn get_client_ids(&self) -> Vec<ClientId> {
        let handles = self.handles.read().await;
        handles.keys().cloned().collect()
    }
}

/// WebSocket Server
pub struct WsServer {
    config: WsServerConfig,
    handler: Arc<dyn ServerHandler>,
    tls_acceptor: TlsAcceptor,
    clients: Arc<tokio::sync::Mutex<Vec<ClientConnection>>>,
    handle_manager: ClientHandleManager,
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
            handle_manager: ClientHandleManager::new(),
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
                    let handle_manager = self.handle_manager.handles.clone();
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
                            handle_manager,
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



    /// Get a handle for a specific client
    ///
    /// This method returns a `ClientHandle` that can be used to interact with
    /// a specific client connection. The handle provides methods for sending
    /// messages and managing client data.
    ///
    /// # Arguments
    ///
    /// * `client_id` - The ID of the client to get a handle for
    ///
    /// # Returns
    ///
    /// * `Option<ClientHandle>` - The client handle if the client is connected, None otherwise
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Get a handle for a specific client
    /// if let Some(handle) = server.get_client_handle(&client_id).await {
    ///     // Send a message to the client
    ///     handle.send_message(WsMessage::Text("Hello!".to_string())).await?;
    /// }
    /// ```
    pub async fn get_client_handle(&self, client_id: &ClientId) -> Option<ClientHandle> {
        self.handle_manager.get_handle(client_id).await
    }

    /// Get handles for all connected clients
    ///
    /// # Returns
    ///
    /// * `Vec<ClientHandle>` - List of handles for all connected clients
    pub async fn get_all_client_handles(&self) -> Vec<ClientHandle> {
        self.handle_manager.get_all_handles().await
    }

    /// Get the number of active client handles
    ///
    /// # Returns
    ///
    /// * `usize` - Number of active client handles
    pub async fn client_handle_count(&self) -> usize {
        self.handle_manager.handle_count().await
    }

    /// Get all client IDs that have active handles
    ///
    /// # Returns
    ///
    /// * `Vec<ClientId>` - List of client IDs with active handles
    pub async fn get_client_handle_ids(&self) -> Vec<ClientId> {
        self.handle_manager.get_client_ids().await
    }

    /// Get the URL suffix for a specific client
    ///
    /// This method returns the URL path suffix that was used when the client connected.
    /// For example, if a client connected to `wss://127.0.0.1:9999/ocpp/CS001`,
    /// this method will return `"ocpp/CS001"`.
    ///
    /// # Arguments
    ///
    /// * `client_id` - The ID of the client to get the URL suffix for
    ///
    /// # Returns
    ///
    /// * `Option<String>` - The URL suffix if the client is connected, None otherwise
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Client connected to: wss://127.0.0.1:9999/ocpp/CS001
    /// if let Some(suffix) = server.get_client_url_suffix(&client_id).await {
    ///     println!("Client URL suffix: {}", suffix); // Prints: "ocpp/CS001"
    /// }
    /// ```
    pub async fn get_client_url_suffix(&self, client_id: &ClientId) -> Option<String> {
        if let Some(handle) = self.get_client_handle(client_id).await {
            Some(handle.path_info().suffix().to_string())
        } else {
            None
        }
    }

    /// Get the decoded URL suffix for a specific client
    ///
    /// This method returns the URL-decoded path suffix that was used when the client connected.
    /// URL encoding like `%7C` will be converted to `|`, `%20` to space, etc.
    /// For example, if a client connected to `wss://127.0.0.1:9999/ocpp/RDAM%7C123`,
    /// this method will return `"ocpp/RDAM|123"`.
    ///
    /// # Arguments
    ///
    /// * `client_id` - The ID of the client to get the decoded URL suffix for
    ///
    /// # Returns
    ///
    /// * `Option<String>` - The decoded URL suffix if the client is connected, None otherwise
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Client connected to: wss://127.0.0.1:9999/ocpp/RDAM%7C123
    /// if let Some(suffix) = server.get_client_url_suffix_decoded(&client_id).await {
    ///     println!("Decoded URL suffix: {}", suffix); // Prints: "ocpp/RDAM|123"
    /// }
    /// ```
    pub async fn get_client_url_suffix_decoded(&self, client_id: &ClientId) -> Option<String> {
        if let Some(handle) = self.get_client_handle(client_id).await {
            Some(handle.path_info().decoded_suffix().to_string())
        } else {
            None
        }
    }

    /// Get the full path information for a specific client
    ///
    /// This method returns the complete `PathInfo` object containing both raw and decoded
    /// path information, along with various utility methods for path analysis.
    ///
    /// # Arguments
    ///
    /// * `client_id` - The ID of the client to get the path information for
    ///
    /// # Returns
    ///
    /// * `Option<PathInfo>` - The path information if the client is connected, None otherwise
    ///
    /// # Example
    ///
    /// ```ignore
    /// if let Some(path_info) = server.get_client_path_info(&client_id).await {
    ///     println!("Raw path: {}", path_info.raw());
    ///     println!("Decoded path: {}", path_info.decoded());
    ///     println!("URL suffix: {}", path_info.suffix());
    ///     println!("Decoded suffix: {}", path_info.decoded_suffix());
    ///     println!("Path segments: {:?}", path_info.segments());
    /// }
    /// ```
    pub async fn get_client_path_info(&self, client_id: &ClientId) -> Option<PathInfo> {
        if let Some(handle) = self.get_client_handle(client_id).await {
            Some(handle.path_info().clone())
        } else {
            None
        }
    }

    /// Get URL suffixes for all connected clients
    ///
    /// This method returns a mapping of client IDs to their URL suffixes.
    ///
    /// # Returns
    ///
    /// * `HashMap<ClientId, String>` - Map of client IDs to their URL suffixes
    ///
    /// # Example
    ///
    /// ```ignore
    /// let client_suffixes = server.get_all_client_url_suffixes().await;
    /// for (client_id, suffix) in client_suffixes {
    ///     println!("Client {} connected with suffix: {}", client_id.0, suffix);
    /// }
    /// ```
    pub async fn get_all_client_url_suffixes(&self) -> std::collections::HashMap<ClientId, String> {
        let mut result = std::collections::HashMap::new();
        let handles = self.get_all_client_handles().await;

        for handle in handles {
            result.insert(handle.client_id().clone(), handle.path_info().suffix().to_string());
        }

        result
    }

    /// Get decoded URL suffixes for all connected clients
    ///
    /// This method returns a mapping of client IDs to their decoded URL suffixes.
    ///
    /// # Returns
    ///
    /// * `HashMap<ClientId, String>` - Map of client IDs to their decoded URL suffixes
    ///
    /// # Example
    ///
    /// ```ignore
    /// let client_suffixes = server.get_all_client_url_suffixes_decoded().await;
    /// for (client_id, suffix) in client_suffixes {
    ///     println!("Client {} connected with decoded suffix: {}", client_id.0, suffix);
    /// }
    /// ```
    pub async fn get_all_client_url_suffixes_decoded(&self) -> std::collections::HashMap<ClientId, String> {
        let mut result = std::collections::HashMap::new();
        let handles = self.get_all_client_handles().await;

        for handle in handles {
            result.insert(handle.client_id().clone(), handle.path_info().decoded_suffix().to_string());
        }

        result
    }

    /// Handle a new WebSocket connection
    async fn handle_connection(
        stream: TcpStream,
        addr: SocketAddr,
        acceptor: TlsAcceptor,
        handler: Arc<dyn ServerHandler>,
        clients: Arc<tokio::sync::Mutex<Vec<ClientConnection>>>,
        handle_manager: Arc<RwLock<HashMap<ClientId, ClientHandle>>>,
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
        
        // Apply timeout to the WebSocket handshake with header callback to extract path
        use std::sync::{Arc, Mutex};
        let connection_path = Arc::new(Mutex::new(String::new()));
        let path_clone = connection_path.clone();

        let callback = move |request: &Request, response: Response| -> Result<Response, ErrorResponse> {
            // Extract the path from the request URI
            let path = request.uri().path().to_string();
            if let Ok(mut path_guard) = path_clone.lock() {
                *path_guard = path.clone();
            }
            debug!("WebSocket connection request path: {}", path);
            Ok(response)
        };

        let ws_stream = tokio::time::timeout(
            connection_timeout,
            accept_hdr_async(tls_handshake, callback),
        ).await
            .map_err(|_| format!("WebSocket handshake timed out after {} seconds", connection_timeout.as_secs()))?
            .map_err(|e| format!("WebSocket handshake failed: {}", e))?;

        let extracted_path = connection_path.lock().unwrap().clone();
        debug!("WebSocket handshake successful for {} with path: {}", addr, extracted_path);
        
        // Create message channels
        let (tx, mut rx) = mpsc::channel::<WsMessage>(100);
        // Create channel for forwarding messages to application layer
        let (app_tx, app_rx) = mpsc::channel::<WsMessage>(100);

        // Register the client
        {
            let mut clients_lock = clients.lock().await;
            clients_lock.push(ClientConnection {
                client_id: client_id.clone(),
                tx: tx.clone(),
            });

            info!("Client connected: {} from {}", client_id.0, addr);
        }

        // Create PathInfo for enhanced path processing
        let path_info = PathInfo::new(extracted_path);
        debug!("Path info created: raw='{}', decoded='{}', suffix='{}'",
               path_info.raw(), path_info.decoded(), path_info.decoded_suffix());

        // Create a client handle and register it
        let client_handle = ClientHandle {
            client_id: client_id.clone(),
            addr,
            path_info: path_info.clone(),
            tx: tx.clone(),
            message_rx: Arc::new(tokio::sync::Mutex::new(app_rx)),
        };

        // Register the handle
        {
            let mut handles = handle_manager.write().await;
            handles.insert(client_id.clone(), client_handle);
        }

        // Notify the handler about the new connection with enhanced path information
        handler.on_connect_with_path(client_id.clone(), addr, path_info);
        
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
            let app_tx_for_recv = app_tx.clone();

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

                            // Forward message to application layer (non-blocking)
                            let _ = app_tx_for_recv.try_send(ws_msg.clone());

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

        // Remove the client handle
        {
            let mut handles = handle_manager.write().await;
            handles.remove(&client_id);
        }

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
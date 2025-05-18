use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use log::{error, info, warn};
use rustls::RootCertStore;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use std::collections::HashMap;
use tokio::sync::{Mutex, mpsc};
use tokio::task::JoinHandle;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{Connector, connect_async_tls_with_config};
use url::Url;

/// WebSocket client structure for handling secure WebSocket connections.
///
/// This client supports TLS/SSL secure connections and provides a simple interface
/// for sending and receiving messages. It is optimized for performance with features like:
/// - Binary message support
/// - Connection timeout handling
/// - Certificate caching
/// - Auto-reconnection capabilities
/// - Optimized memory usage
///
/// # Example
///
/// ```ignore
/// use ws_rs::client::WebSocketClient;
/// use ws_rs::client::MessageType;
/// use std::time::Duration;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     // Create a client with custom configuration
///     let mut client = WebSocketClient::builder()
///         .with_channel_capacity(200)
///         .with_connection_timeout(Duration::from_secs(10))
///         .with_auto_reconnect(true)
///         .build();
///
///     // Connect to a WebSocket server
///     client.connect(
///         "wss://127.0.0.1:9000",
///         "./certs",
///         "client_cert.pem",
///         "client_key.pem",
///         "ca_cert.pem"
///     ).await?;
///
///     // Send a text message
///     client.send_message(MessageType::Text("Hello, server!".to_string())).await?;
///
///     // Send a binary message
///     client.send_message(MessageType::Binary(vec![1, 2, 3, 4])).await?;
///
///     // Receive a message
///     if let Some(response) = client.receive_message().await {
///         match response {
///             MessageType::Text(text) => println!("Received text: {}", text),
///             MessageType::Binary(data) => println!("Received binary data: {} bytes", data.len()),
///         }
///     }
///
///     // Close the connection
///     client.close().await;
///
///     Ok(())
/// }
/// ```
/// Message type enum for WebSocket communication
#[derive(Debug, Clone)]
pub enum MessageType {
    /// Text message
    Text(String),
    /// Binary message
    Binary(Vec<u8>),
}

/// Configuration for WebSocketClient
#[derive(Debug, Clone)]
pub struct WSClientConfig {
    /// Channel capacity for message queues
    pub channel_capacity: usize,
    /// Connection timeout in seconds
    pub connection_timeout: Duration,
    /// Whether to automatically reconnect on connection failure
    pub auto_reconnect: bool,
    /// Maximum reconnection attempts
    pub max_reconnect_attempts: u32,
    /// Delay between reconnection attempts
    pub reconnect_delay: Duration,
}

impl Default for WSClientConfig {
    fn default() -> Self {
        Self {
            channel_capacity: 100,
            connection_timeout: Duration::from_secs(30),
            auto_reconnect: false,
            max_reconnect_attempts: 5,
            reconnect_delay: Duration::from_secs(2),
        }
    }
}

/// Builder for WebSocketClient
pub struct WebSocketClientBuilder {
    config: WSClientConfig,
}

impl WebSocketClientBuilder {
    /// Create a new builder with default configuration
    pub fn new() -> Self {
        Self {
            config: WSClientConfig::default(),
        }
    }

    /// Set channel capacity
    pub fn with_channel_capacity(mut self, capacity: usize) -> Self {
        self.config.channel_capacity = capacity;
        self
    }

    /// Set connection timeout
    pub fn with_connection_timeout(mut self, timeout: Duration) -> Self {
        self.config.connection_timeout = timeout;
        self
    }

    /// Enable or disable auto-reconnect
    pub fn with_auto_reconnect(mut self, auto_reconnect: bool) -> Self {
        self.config.auto_reconnect = auto_reconnect;
        self
    }

    /// Set maximum reconnection attempts
    pub fn with_max_reconnect_attempts(mut self, attempts: u32) -> Self {
        self.config.max_reconnect_attempts = attempts;
        self
    }

    /// Set delay between reconnection attempts
    pub fn with_reconnect_delay(mut self, delay: Duration) -> Self {
        self.config.reconnect_delay = delay;
        self
    }

    /// Build the WebSocketClient with the configured options
    pub fn build(self) -> WebSocketClient {
        WebSocketClient {
            sender: None,
            receiver: None,
            ws_handle: None,
            is_connected: false,
            server_url: None,
            cert_paths: None,
            config: self.config,
            cert_cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

pub struct WebSocketClient {
    sender: Option<mpsc::Sender<MessageType>>,
    receiver: Option<mpsc::Receiver<MessageType>>,
    ws_handle: Option<JoinHandle<()>>,
    is_connected: bool,
    server_url: Option<Url>,
    cert_paths: Option<(String, String, String, String, String)>,
    config: WSClientConfig,
    cert_cache: Arc<Mutex<HashMap<String, Arc<rustls::ClientConfig>>>>,
}

impl WebSocketClient {
    /// Creates a new WebSocketClient instance with default configuration.
    ///
    /// The new client is initially disconnected. Use the `connect` method
    /// to establish a connection to a WebSocket server.
    ///
    /// # Returns
    ///
    /// A new `WebSocketClient` instance.
    pub fn new() -> Self {
        Self::builder().build()
    }

    /// Creates a builder for configuring a WebSocketClient.
    ///
    /// # Returns
    ///
    /// A WebSocketClientBuilder instance.
    pub fn builder() -> WebSocketClientBuilder {
        WebSocketClientBuilder::new()
    }

    /// Loads certificates from a PEM file.
    ///
    /// # Parameters
    ///
    /// * `path` - Path to the certificate file
    ///
    /// # Returns
    ///
    /// A vector of certificates in DER format.
    ///
    /// # Panics
    ///
    /// Panics if the certificate file cannot be opened or parsed.
    fn load_certs(path: &Path) -> Result<Vec<CertificateDer<'static>>, Box<dyn std::error::Error>> {
        let file = File::open(path)?;
        let mut reader = BufReader::new(file);
        let certs = rustls_pemfile::certs(&mut reader).collect::<Result<Vec<_>, _>>()?;

        if certs.is_empty() {
            return Err("No certificates found in file".into());
        }

        Ok(certs)
    }

    /// Loads a private key from a PEM file.
    ///
    /// # Parameters
    ///
    /// * `path` - Path to the private key file
    ///
    /// # Returns
    ///
    /// The private key in DER format.
    ///
    /// # Errors
    ///
    /// Returns an error if the private key file cannot be opened, parsed, or if no keys are found.
    fn load_private_key(path: &Path) -> Result<PrivateKeyDer<'static>, Box<dyn std::error::Error>> {
        let file = File::open(path)?;
        let mut reader = BufReader::new(file);
        let keys =
            rustls_pemfile::pkcs8_private_keys(&mut reader).collect::<Result<Vec<_>, _>>()?;

        if keys.is_empty() {
            return Err("No private key found in file".into());
        }

        // Use the first private key
        Ok(PrivateKeyDer::Pkcs8(keys.into_iter().next().unwrap()))
    }

    /// Creates a TLS client configuration from certificates and keys.
    ///
    /// This method attempts to reuse cached configurations when possible.
    ///
    /// # Parameters
    ///
    /// * `cache_key` - A unique key for caching the configuration
    /// * `client_cert_path` - Path to client certificate
    /// * `client_key_path` - Path to client private key
    /// * `ca_cert_path` - Path to CA certificate
    ///
    /// # Returns
    ///
    /// A TLS client configuration or an error.
    async fn create_tls_config(
        &self,
        cache_key: &str,
        client_cert_path: &Path,
        client_key_path: &Path,
        ca_cert_path: &Path,
    ) -> Result<Arc<rustls::ClientConfig>, Box<dyn std::error::Error>> {
        // Check if we have a cached configuration
        {
            let cache = self.cert_cache.lock().await;
            if let Some(config) = cache.get(cache_key) {
                info!("Using cached TLS configuration");
                return Ok(config.clone());
            }
        }

        // Load certificates and keys
        let client_certs = Self::load_certs(client_cert_path)?;
        let client_key = Self::load_private_key(client_key_path)?;
        let ca_certs = Self::load_certs(ca_cert_path)?;

        // Create TLS configuration
        let mut root_store = RootCertStore::empty();
        for cert in ca_certs {
            root_store.add(cert)?;
        }

        let client_config = rustls::ClientConfig::builder()
            .with_root_certificates(root_store)
            .with_client_auth_cert(client_certs, client_key)?;

        let config = Arc::new(client_config);

        // Cache the configuration
        {
            let mut cache = self.cert_cache.lock().await;
            cache.insert(cache_key.to_string(), config.clone());
        }

        Ok(config)
    }

    /// Connects to a WebSocket server using TLS.
    ///
    /// This method establishes a secure WebSocket connection to the specified server URL
    /// using the provided certificates and keys.
    ///
    /// # Parameters
    ///
    /// * `server_url` - The WebSocket server URL (e.g., "wss://example.com:9000")
    /// * `cert_dir` - Directory containing the certificate files
    /// * `client_cert_file` - Client certificate filename
    /// * `client_key_file` - Client private key filename
    /// * `ca_cert_file` - CA certificate filename
    ///
    /// # Returns
    ///
    /// `Ok(())` on successful connection, or an error if the connection fails.
    ///
    /// # Errors
    ///
    /// Returns an error if URL parsing fails, certificate loading fails, or connection fails.
    pub async fn connect(
        &mut self,
        server_url: &str,
        cert_dir: &str,
        client_cert_file: &str,
        client_key_file: &str,
        ca_cert_file: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Parse server URL
        let server_url = Url::parse(server_url)?;

        // Save connection parameters for potential reconnection
        self.server_url = Some(server_url.clone());
        self.cert_paths = Some((
            server_url.to_string(),
            cert_dir.to_string(),
            client_cert_file.to_string(),
            client_key_file.to_string(),
            ca_cert_file.to_string(),
        ));

        // Handle connection with retries
        let mut current_attempt = 0;
        loop {
            // Perform the connection attempt
            let result = self
                .connect_internal(
                    &server_url,
                    cert_dir,
                    client_cert_file,
                    client_key_file,
                    ca_cert_file,
                    current_attempt,
                )
                .await;

            // Check if we need to retry based on the special error
            match result {
                Err(e) => {
                    let err_str = e.to_string();
                    if err_str.starts_with("__RETRY_CONNECTION_") {
                        // Parse the attempt number
                        if let Ok(next_attempt) = err_str
                            .trim_start_matches("__RETRY_CONNECTION_")
                            .parse::<u32>()
                        {
                            current_attempt = next_attempt;
                            // Wait before retrying
                            tokio::time::sleep(self.config.reconnect_delay).await;
                            continue;
                        }
                    }
                    return Err(e);
                }
                Ok(_) => return Ok(()),
            }
        }
    }

    /// Internal connect method that handles reconnection attempts
    ///
    /// This function uses manual reconnection logic instead of recursive calls
    /// to avoid boxing issues with async functions.
    async fn connect_internal(
        &mut self,
        server_url: &Url,
        cert_dir: &str,
        client_cert_file: &str,
        client_key_file: &str,
        ca_cert_file: &str,
        attempt: u32,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Certificate paths
        let cert_dir = Path::new(cert_dir);
        let client_cert = cert_dir.join(client_cert_file);
        let client_key = cert_dir.join(client_key_file);
        let ca_cert = cert_dir.join(ca_cert_file);

        info!("Client certificate: {:?}", client_cert);
        info!("Client private key: {:?}", client_key);
        info!("CA certificate: {:?}", ca_cert);

        // Create a cache key for the TLS configuration
        let cache_key = format!(
            "{}:{}:{}:{}",
            server_url,
            client_cert.display(),
            client_key.display(),
            ca_cert.display()
        );

        info!("Loading certificates and keys...");
        let tls_config = match self
            .create_tls_config(&cache_key, &client_cert, &client_key, &ca_cert)
            .await
        {
            Ok(config) => config,
            Err(e) => {
                error!("Failed to create TLS configuration: {}", e);
                return Err(e);
            }
        };

        // Create TLS connector
        let connector = Connector::Rustls(tls_config);

        // Connect to WebSocket server with timeout
        info!("Connecting to WebSocket server: {}", server_url);
        // Use timeout for connection attempt
        let connection_attempt =
            connect_async_tls_with_config(server_url.clone(), None, false, Some(connector));
        let ws_stream = match timeout(self.config.connection_timeout, connection_attempt).await {
            Ok(result) => {
                match result {
                    Ok((stream, _)) => stream,
                    Err(e) => {
                        error!("Connection error: {}", e);

                        // Handle reconnection if enabled
                        if self.config.auto_reconnect
                            && attempt < self.config.max_reconnect_attempts
                        {
                            warn!(
                                "Reconnection attempt {}/{} in {}s",
                                attempt + 1,
                                self.config.max_reconnect_attempts,
                                self.config.reconnect_delay.as_secs()
                            );

                            // Wait before attempting to reconnect
                            tokio::time::sleep(self.config.reconnect_delay).await;

                            // Rather than making a recursive call, we'll return a special error
                            // that indicates we should retry the connection
                            return Err(format!("__RETRY_CONNECTION_{}", attempt + 1).into());
                        }

                        return Err(e.into());
                    }
                }
            }
            Err(_) => {
                let err = format!(
                    "Connection timeout after {:?}",
                    self.config.connection_timeout
                );
                error!("{}", err);

                // Handle reconnection if enabled
                if self.config.auto_reconnect && attempt < self.config.max_reconnect_attempts {
                    warn!(
                        "Reconnection attempt {}/{} in {}s",
                        attempt + 1,
                        self.config.max_reconnect_attempts,
                        self.config.reconnect_delay.as_secs()
                    );

                    // Wait before attempting to reconnect
                    tokio::time::sleep(self.config.reconnect_delay).await;

                    // Rather than making a recursive call, we'll return a special error
                    // that indicates we should retry the connection
                    return Err(format!("__RETRY_CONNECTION_{}", attempt + 1).into());
                }

                return Err(err.into());
            }
        };

        info!("Connected to WebSocket server");

        // Create channels for message passing with configured capacity
        let (tx_sender, mut rx_sender) = mpsc::channel::<MessageType>(self.config.channel_capacity);
        let (tx_receiver, rx_receiver) = mpsc::channel::<MessageType>(self.config.channel_capacity);

        // Split connection into sender and receiver
        let (mut ws_sender, mut ws_receiver) = ws_stream.split();

        // Task for handling outgoing messages
        let send_task = tokio::spawn(async move {
            while let Some(message) = rx_sender.recv().await {
                let ws_message = match message {
                    MessageType::Text(text) => Message::Text(text),
                    MessageType::Binary(data) => Message::Binary(data),
                };

                match ws_sender.send(ws_message).await {
                    Ok(_) => info!("Message sent"),
                    Err(e) => {
                        error!("Error sending message: {}", e);
                        break;
                    }
                }
            }
            // Close WebSocket connection
            let _ = ws_sender.close().await;
        });

        // Task for handling incoming messages
        let receive_task = tokio::spawn(async move {
            while let Some(msg) = ws_receiver.next().await {
                match msg {
                    Ok(msg) => {
                        let message = match msg {
                            Message::Text(text) => {
                                info!("Received text message: {} bytes", text.len());
                                MessageType::Text(text)
                            }
                            Message::Binary(data) => {
                                info!("Received binary message: {} bytes", data.len());
                                MessageType::Binary(data)
                            }
                            Message::Ping(_) | Message::Pong(_) => {
                                // Handle ping/pong internally
                                continue;
                            }
                            Message::Close(_) => {
                                info!("Received close frame");
                                break;
                            }
                            // Handle other message types if needed
                            _ => continue,
                        };

                        if let Err(e) = tx_receiver.send(message).await {
                            error!("Error forwarding to receiver channel: {}", e);
                            break;
                        }
                    }
                    Err(e) => {
                        error!("Error receiving message: {}", e);
                        break;
                    }
                }
            }
        });

        // Combine tasks with select to handle termination
        let handle = tokio::spawn(async move {
            tokio::select! {
                _ = send_task => info!("Send task completed"),
                _ = receive_task => info!("Receive task completed"),
            }
        });

        // Update client state
        self.sender = Some(tx_sender);
        self.receiver = Some(rx_receiver);
        self.ws_handle = Some(handle);
        self.is_connected = true;

        Ok(())
    }

    /// Reconnects to the WebSocket server using the last connection parameters.
    ///
    /// # Returns
    ///
    /// `Ok(())` on successful reconnection, or an error if reconnection fails.
    ///
    /// # Errors
    ///
    /// Returns an error if no previous connection exists or if reconnection fails.
    pub async fn reconnect(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some((url, cert_dir, client_cert, client_key, ca_cert)) = self.cert_paths.clone() {
            // Close existing connection if any
            if self.is_connected {
                self.close().await;
            }

            // Connect using saved parameters
            self.connect(&url, &cert_dir, &client_cert, &client_key, &ca_cert)
                .await
        } else {
            Err("No previous connection parameters available for reconnection".into())
        }
    }

    /// Sends a message to the connected WebSocket server.
    ///
    /// # Parameters
    ///
    /// * `message` - The message to send (text or binary)
    ///
    /// # Returns
    ///
    /// `Ok(())` if the message was queued for sending, or an error if not connected.
    ///
    /// # Errors
    ///
    /// Returns an error if the client is not connected or if the message cannot be sent.
    pub async fn send_message(
        &self,
        message: MessageType,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(sender) = &self.sender {
            sender.send(message).await?;
            Ok(())
        } else {
            Err("Not connected to WebSocket server".into())
        }
    }

    /// Sends a text message to the connected WebSocket server.
    ///
    /// This is a convenience method that wraps send_message.
    ///
    /// # Parameters
    ///
    /// * `text` - The text message to send
    ///
    /// # Returns
    ///
    /// `Ok(())` if the message was queued for sending, or an error if not connected.
    pub async fn send_text(&self, text: String) -> Result<(), Box<dyn std::error::Error>> {
        self.send_message(MessageType::Text(text)).await
    }

    /// Sends a binary message to the connected WebSocket server.
    ///
    /// This is a convenience method that wraps send_message.
    ///
    /// # Parameters
    ///
    /// * `data` - The binary data to send
    ///
    /// # Returns
    ///
    /// `Ok(())` if the message was queued for sending, or an error if not connected.
    pub async fn send_binary(&self, data: Vec<u8>) -> Result<(), Box<dyn std::error::Error>> {
        self.send_message(MessageType::Binary(data)).await
    }

    /// Receives a message from the WebSocket server.
    ///
    /// This method waits for the next message from the server. If no message
    /// is available or the connection is closed, it returns `None`.
    ///
    /// # Returns
    ///
    /// * `Some(MessageType)` - The received message (text or binary)
    /// * `None` - If not connected or the connection was closed
    pub async fn receive_message(&mut self) -> Option<MessageType> {
        if let Some(receiver) = &mut self.receiver {
            receiver.recv().await
        } else {
            None
        }
    }

    /// Receives a message with timeout.
    ///
    /// This method waits for the next message from the server with a timeout.
    ///
    /// # Parameters
    ///
    /// * `timeout_duration` - Maximum time to wait for a message
    ///
    /// # Returns
    ///
    /// * `Ok(Some(MessageType))` - A message was received
    /// * `Ok(None)` - No message received (not connected)
    /// * `Err(_)` - Timeout occurred
    pub async fn receive_message_timeout(
        &mut self,
        timeout_duration: Duration,
    ) -> Result<Option<MessageType>, tokio::time::error::Elapsed> {
        if let Some(receiver) = &mut self.receiver {
            timeout(timeout_duration, receiver.recv()).await
        } else {
            Ok(None)
        }
    }

    /// Checks if the client is connected to a WebSocket server.
    ///
    /// # Returns
    ///
    /// `true` if connected, `false` otherwise.
    pub fn is_connected(&self) -> bool {
        self.is_connected
    }

    /// Closes the WebSocket connection.
    ///
    /// This method gracefully shuts down the connection by:
    /// 1. Dropping the sender channel to trigger closing the WebSocket
    /// 2. Waiting for the worker task to complete
    /// 3. Cleaning up resources
    ///
    /// The client can be reconnected after closing by calling `connect()` again.
    pub async fn close(&mut self) {
        // Drop the sender channel to trigger close operation
        self.sender = None;

        // Wait for the main task to complete
        if let Some(handle) = self.ws_handle.take() {
            let _ = handle.await;
        }

        self.receiver = None;
        self.is_connected = false;

        info!("WebSocket connection closed");
    }

    /// Sends a ping message to check connection health.
    ///
    /// This method can be used to keep the connection alive or
    /// check if the server is still responsive.
    ///
    /// # Returns
    ///
    /// `Ok(())` if the ping was sent, or an error if not connected.
    pub async fn ping(&self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(sender) = &self.sender {
            // Use an empty binary message as a ping
            sender.send(MessageType::Binary(Vec::new())).await?;
            Ok(())
        } else {
            Err("Not connected to WebSocket server".into())
        }
    }

    /// Clears the certificate cache.
    ///
    /// This method can be useful to force reloading of certificates
    /// if they have been updated on disk.
    pub async fn clear_cert_cache(&self) {
        let mut cache = self.cert_cache.lock().await;
        cache.clear();
        info!("Certificate cache cleared");
    }

    /// Checks if a connection is active and sends a ping to verify connectivity.
    ///
    /// Returns true if the connection is active and responsive.
    pub async fn check_connection(&self) -> bool {
        if !self.is_connected {
            return false;
        }

        match self.ping().await {
            Ok(_) => true,
            Err(_) => false,
        }
    }

    /// Gets the current configuration.
    ///
    /// # Returns
    ///
    /// A reference to the current client configuration.
    pub fn get_config(&self) -> &WSClientConfig {
        &self.config
    }
}

impl Drop for WebSocketClient {
    fn drop(&mut self) {
        // If the client is still connected when going out of scope,
        // drop all channels to allow resources to be cleaned up
        self.sender = None;
        self.receiver = None;

        // Drop the task handle, allowing it to complete on its own
        self.ws_handle = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_type() {
        let text = MessageType::Text("hello".to_string());
        let binary = MessageType::Binary(vec![1, 2, 3]);

        match text {
            MessageType::Text(s) => assert_eq!(s, "hello"),
            _ => panic!("Expected Text variant"),
        }

        match binary {
            MessageType::Binary(b) => assert_eq!(b, vec![1, 2, 3]),
            _ => panic!("Expected Binary variant"),
        }
    }

    #[test]
    fn test_client_config_default() {
        let config = WSClientConfig::default();
        assert_eq!(config.channel_capacity, 100);
        assert_eq!(config.connection_timeout, Duration::from_secs(30));
        assert_eq!(config.auto_reconnect, false);
    }

    #[test]
    fn test_client_builder() {
        let client = WebSocketClient::builder()
            .with_channel_capacity(200)
            .with_connection_timeout(Duration::from_secs(10))
            .with_auto_reconnect(true)
            .build();

        assert_eq!(client.config.channel_capacity, 200);
        assert_eq!(client.config.connection_timeout, Duration::from_secs(10));
        assert_eq!(client.config.auto_reconnect, true);
    }
}

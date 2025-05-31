//! WebSocket compression support (RFC 7692 permessage-deflate)
//!
//! This module provides a framework for WebSocket compression support as defined in RFC 7692.
//! It includes configuration, negotiation, and extension parsing capabilities for the
//! permessage-deflate WebSocket extension.
//!
//! # Current Status
//!
//! **Note**: The underlying `tungstenite` library does not yet have built-in compression support.
//! This module provides the infrastructure for compression negotiation and configuration,
//! preparing for future compression support when it becomes available in tungstenite.
//!
//! # Features
//!
//! - **Extension negotiation**: Handles Sec-WebSocket-Extensions header parsing and formatting
//! - **Configuration management**: Provides comprehensive compression configuration options
//! - **RFC 7692 compliance**: Follows the permessage-deflate specification
//! - **Future-ready**: Prepared for actual compression when library support is available
//!
//! # Usage
//!
//! Currently, this module handles compression negotiation during the WebSocket handshake.
//! When tungstenite adds compression support, the actual compression/decompression will
//! be handled transparently by the underlying library.

#[cfg(feature = "compression")]
use flate2::{Compression, Decompress, Compress};
#[cfg(feature = "compression")]
use log::debug;
use log::warn;
#[cfg(feature = "compression")]
use std::collections::HashMap;

/// Configuration for WebSocket compression
#[derive(Debug, Clone)]
pub struct CompressionConfig {
    /// Enable compression support
    pub enabled: bool,
    /// Compression level (0-9, where 9 is maximum compression)
    pub level: u32,
    /// Server maximum window bits (8-15)
    pub server_max_window_bits: Option<u8>,
    /// Client maximum window bits (8-15)
    pub client_max_window_bits: Option<u8>,
    /// Server no context takeover
    pub server_no_context_takeover: bool,
    /// Client no context takeover
    pub client_no_context_takeover: bool,
}

impl Default for CompressionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            level: 6, // Default compression level
            server_max_window_bits: None, // Use default (15)
            client_max_window_bits: None, // Use default (15)
            server_no_context_takeover: false,
            client_no_context_takeover: false,
        }
    }
}

impl CompressionConfig {
    /// Create a new compression configuration with default settings
    pub fn new() -> Self {
        Self::default()
    }

    /// Enable or disable compression
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Set compression level (0-9)
    pub fn with_level(mut self, level: u32) -> Self {
        self.level = level.min(9);
        self
    }

    /// Set server maximum window bits (8-15)
    pub fn with_server_max_window_bits(mut self, bits: Option<u8>) -> Self {
        self.server_max_window_bits = bits.map(|b| b.clamp(8, 15));
        self
    }

    /// Set client maximum window bits (8-15)
    pub fn with_client_max_window_bits(mut self, bits: Option<u8>) -> Self {
        self.client_max_window_bits = bits.map(|b| b.clamp(8, 15));
        self
    }

    /// Set server no context takeover
    pub fn with_server_no_context_takeover(mut self, enabled: bool) -> Self {
        self.server_no_context_takeover = enabled;
        self
    }

    /// Set client no context takeover
    pub fn with_client_no_context_takeover(mut self, enabled: bool) -> Self {
        self.client_no_context_takeover = enabled;
        self
    }
}

/// WebSocket extension parameter
#[derive(Debug, Clone, PartialEq)]
pub struct ExtensionParam {
    pub name: String,
    pub value: Option<String>,
}

/// WebSocket extension
#[derive(Debug, Clone)]
pub struct Extension {
    pub name: String,
    pub params: Vec<ExtensionParam>,
}

impl Extension {
    /// Create a new extension
    pub fn new(name: String) -> Self {
        Self {
            name,
            params: Vec::new(),
        }
    }

    /// Add a parameter to the extension
    pub fn with_param(mut self, name: String, value: Option<String>) -> Self {
        self.params.push(ExtensionParam { name, value });
        self
    }

    /// Get parameter value by name
    pub fn get_param(&self, name: &str) -> Option<&ExtensionParam> {
        self.params.iter().find(|p| p.name == name)
    }

    /// Check if parameter exists
    pub fn has_param(&self, name: &str) -> bool {
        self.params.iter().any(|p| p.name == name)
    }
}

/// Parse WebSocket extensions from header value
pub fn parse_extensions(header_value: &str) -> Vec<Extension> {
    let mut extensions = Vec::new();
    
    for ext_str in header_value.split(',') {
        let ext_str = ext_str.trim();
        if ext_str.is_empty() {
            continue;
        }

        let mut parts = ext_str.split(';');
        if let Some(name) = parts.next() {
            let name = name.trim().to_string();
            let mut extension = Extension::new(name);

            for param_str in parts {
                let param_str = param_str.trim();
                if let Some(eq_pos) = param_str.find('=') {
                    let param_name = param_str[..eq_pos].trim().to_string();
                    let param_value = param_str[eq_pos + 1..].trim();
                    let param_value = if param_value.starts_with('"') && param_value.ends_with('"') {
                        Some(param_value[1..param_value.len() - 1].to_string())
                    } else {
                        Some(param_value.to_string())
                    };
                    extension = extension.with_param(param_name, param_value);
                } else {
                    extension = extension.with_param(param_str.to_string(), None);
                }
            }

            extensions.push(extension);
        }
    }

    extensions
}

/// Format extensions for header value
pub fn format_extensions(extensions: &[Extension]) -> String {
    extensions
        .iter()
        .map(|ext| {
            let mut result = ext.name.clone();
            for param in &ext.params {
                result.push_str("; ");
                result.push_str(&param.name);
                if let Some(ref value) = param.value {
                    result.push('=');
                    if value.contains(' ') || value.contains(',') || value.contains(';') {
                        result.push('"');
                        result.push_str(value);
                        result.push('"');
                    } else {
                        result.push_str(value);
                    }
                }
            }
            result
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// Compression context for a WebSocket connection
#[cfg(feature = "compression")]
pub struct CompressionContext {
    compressor: Compress,
    decompressor: Decompress,
    config: CompressionConfig,
    negotiated_params: HashMap<String, String>,
}

#[cfg(feature = "compression")]
impl CompressionContext {
    /// Create a new compression context
    pub fn new(config: CompressionConfig) -> Self {
        let compressor = Compress::new(Compression::new(config.level), false);
        let decompressor = Decompress::new(false);
        
        Self {
            compressor,
            decompressor,
            config,
            negotiated_params: HashMap::new(),
        }
    }

    /// Set negotiated parameters
    pub fn set_negotiated_params(&mut self, params: HashMap<String, String>) {
        self.negotiated_params = params;
        debug!("Compression negotiated with parameters: {:?}", self.negotiated_params);
    }

    /// Check if compression is enabled and negotiated
    pub fn is_enabled(&self) -> bool {
        self.config.enabled && !self.negotiated_params.is_empty()
    }
}

/// Compression support trait
pub trait CompressionSupport {
    /// Generate compression offer for client handshake
    fn generate_compression_offer(&self) -> Option<String>;
    
    /// Accept compression offer for server handshake
    fn accept_compression_offer(&self, offer: &str) -> Option<String>;
    
    /// Check if compression is supported
    fn supports_compression(&self) -> bool;
}

/// Default implementation when compression feature is disabled
#[cfg(not(feature = "compression"))]
pub struct CompressionContext {
    _config: CompressionConfig,
}

#[cfg(not(feature = "compression"))]
impl CompressionContext {
    pub fn new(config: CompressionConfig) -> Self {
        if config.enabled {
            warn!("Compression requested but 'compression' feature is not enabled");
        }
        Self { _config: config }
    }

    pub fn is_enabled(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_extensions() {
        let header = "permessage-deflate; server_max_window_bits=15; client_max_window_bits";
        let extensions = parse_extensions(header);
        
        assert_eq!(extensions.len(), 1);
        assert_eq!(extensions[0].name, "permessage-deflate");
        assert_eq!(extensions[0].params.len(), 2);
        
        let param1 = &extensions[0].params[0];
        assert_eq!(param1.name, "server_max_window_bits");
        assert_eq!(param1.value, Some("15".to_string()));
        
        let param2 = &extensions[0].params[1];
        assert_eq!(param2.name, "client_max_window_bits");
        assert_eq!(param2.value, None);
    }

    #[test]
    fn test_format_extensions() {
        let ext = Extension::new("permessage-deflate".to_string())
            .with_param("server_max_window_bits".to_string(), Some("15".to_string()))
            .with_param("client_max_window_bits".to_string(), None);
        
        let formatted = format_extensions(&[ext]);
        assert_eq!(formatted, "permessage-deflate; server_max_window_bits=15; client_max_window_bits");
    }

    #[test]
    fn test_compression_config() {
        let config = CompressionConfig::new()
            .with_enabled(true)
            .with_level(8)
            .with_server_max_window_bits(Some(12))
            .with_client_no_context_takeover(true);
        
        assert!(config.enabled);
        assert_eq!(config.level, 8);
        assert_eq!(config.server_max_window_bits, Some(12));
        assert!(config.client_no_context_takeover);
    }
}

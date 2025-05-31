pub mod server;

#[cfg(feature = "client")]
pub mod client;

/// URL path processing utilities
pub mod path_utils;

/// WebSocket compression support (RFC 7692)
pub mod compression;

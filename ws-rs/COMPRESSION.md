# WebSocket Compression Support (RFC 7692)

This document describes the WebSocket compression support implementation in the `ws-rs` crate, following RFC 7692 permessage-deflate specification.

## Current Status

⚠️ **Important Note**: The underlying `tungstenite` library does not yet have built-in compression support. This implementation provides the infrastructure for compression negotiation and configuration, preparing for future compression support when it becomes available in tungstenite.

### What's Implemented

✅ **Compression Configuration**: Complete configuration system for permessage-deflate parameters
✅ **Extension Negotiation**: Proper parsing and formatting of `Sec-WebSocket-Extensions` headers
✅ **Server-side Negotiation**: Server accepts and responds to client compression offers
✅ **Client-side Negotiation**: Client sends compression offers and handles server responses
✅ **RFC 7692 Compliance**: Follows the permessage-deflate specification
✅ **Fallback Support**: Graceful degradation when compression is not supported

### What's Not Yet Available

❌ **Actual Compression**: Message compression/decompression (waiting for tungstenite support)
❌ **Window Size Control**: Dynamic window size adjustment (requires tungstenite support)
❌ **Context Takeover**: Actual context management (requires tungstenite support)

## Configuration

### Server Configuration

```rust
use ws_rs::compression::CompressionConfig;
use ws_rs::server::WsServerConfig;

let config = WsServerConfig {
    // ... other config
    compression: CompressionConfig::new()
        .with_enabled(true)                           // Enable compression negotiation
        .with_level(6)                               // Compression level (0-9)
        .with_server_max_window_bits(Some(15))       // Server compression window
        .with_client_max_window_bits(Some(15))       // Allow client maximum window
        .with_server_no_context_takeover(false)      // Allow server context takeover
        .with_client_no_context_takeover(false),     // Allow client context takeover
};
```

### Client Configuration

```rust
use ws_rs::compression::CompressionConfig;
use ws_rs::client::WebSocketClient;

let compression_config = CompressionConfig::new()
    .with_enabled(true)                              // Enable compression negotiation
    .with_level(6)                                   // Compression level (0-9)
    .with_client_max_window_bits(Some(15))           // Request maximum window size
    .with_server_max_window_bits(Some(15))           // Accept server maximum window
    .with_client_no_context_takeover(false)          // Allow client context takeover
    .with_server_no_context_takeover(false);         // Allow server context takeover

let client = WebSocketClient::builder()
    .with_compression(compression_config)
    .build();
```

## Compression Parameters

### Window Bits
- **Range**: 8-15 bits
- **Default**: 15 (maximum compression window)
- **Effect**: Larger windows provide better compression but use more memory

### Compression Level
- **Range**: 0-9
- **0**: No compression (fastest)
- **1**: Best speed
- **6**: Balanced (default)
- **9**: Best compression (slowest)

### Context Takeover
- **Enabled**: Compression context is preserved between messages (better compression)
- **Disabled**: Each message is compressed independently (lower memory usage)

## Extension Negotiation

### Client Request
```
Sec-WebSocket-Extensions: permessage-deflate; client_max_window_bits; server_max_window_bits=15
```

### Server Response
```
Sec-WebSocket-Extensions: permessage-deflate; server_max_window_bits=15; client_max_window_bits=15
```

## Benefits for OCPP

WebSocket compression is particularly beneficial for OCPP (Open Charge Point Protocol) communications:

1. **Reduced Bandwidth**: JSON messages compress very well (typically 60-80% reduction)
2. **Lower Costs**: Reduced mobile data usage for charging stations
3. **Better Performance**: Faster message transmission over slow connections
4. **Standard Compliance**: RFC 7692 is widely supported by WebSocket implementations

## Future Roadmap

When tungstenite adds compression support:

1. **Automatic Activation**: Compression will work transparently with existing configuration
2. **Performance Optimization**: Fine-tuning compression parameters for OCPP workloads
3. **Memory Management**: Efficient handling of compression contexts
4. **Monitoring**: Compression ratio and performance metrics

## Examples

See the `ws_server` and `ws_client` examples for complete usage demonstrations:

```bash
# Run server with compression
cargo run --bin ws_server --features compression

# Run client with compression
cargo run --bin ws_client --features compression
```

## Testing

The compression negotiation can be tested with any WebSocket client that supports permessage-deflate:

```bash
# Test with websocat (if it supports compression)
websocat wss://127.0.0.1:9999/ocpp/CS001 --header="Sec-WebSocket-Extensions: permessage-deflate"
```

## References

- [RFC 7692: Compression Extensions for WebSocket](https://tools.ietf.org/html/rfc7692)
- [OCPP 2.1 Specification](https://www.openchargealliance.org/protocols/ocpp-201/)
- [tungstenite-rs GitHub](https://github.com/snapview/tungstenite-rs)
- [Compression PR #426](https://github.com/snapview/tungstenite-rs/pull/426)

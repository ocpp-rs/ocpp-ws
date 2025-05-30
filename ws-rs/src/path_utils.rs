use percent_encoding::percent_decode_str;
use log::{debug, warn};

/// URL path processing result
#[derive(Debug, Clone, PartialEq)]
pub struct PathInfo {
    /// Original path (not decoded)
    pub raw_path: String,
    /// Decoded path
    pub decoded_path: String,
    /// Suffix (with leading '/' removed)
    pub suffix: String,
    /// Decoded suffix
    pub decoded_suffix: String,
}

impl PathInfo {
    /// Create a new PathInfo instance
    pub fn new(raw_path: String) -> Self {
        let decoded_path = decode_url_path(&raw_path);
        let suffix = extract_suffix(&raw_path);
        let decoded_suffix = decode_url_path(&suffix);

        Self {
            raw_path,
            decoded_path,
            suffix,
            decoded_suffix,
        }
    }

    /// Get the original path
    pub fn raw(&self) -> &str {
        &self.raw_path
    }

    /// Get the decoded path
    pub fn decoded(&self) -> &str {
        &self.decoded_path
    }

    /// Get the original suffix
    pub fn suffix(&self) -> &str {
        &self.suffix
    }

    /// Get the decoded suffix
    pub fn decoded_suffix(&self) -> &str {
        &self.decoded_suffix
    }

    /// Check if the path is empty or root path
    pub fn is_root(&self) -> bool {
        self.suffix.is_empty()
    }

    /// Check if the path starts with the specified prefix (using decoded path)
    pub fn starts_with(&self, prefix: &str) -> bool {
        self.decoded_suffix.starts_with(prefix)
    }

    /// Extract path segments (split by '/')
    pub fn segments(&self) -> Vec<&str> {
        if self.decoded_suffix.is_empty() {
            Vec::new()
        } else {
            self.decoded_suffix.split('/').collect()
        }
    }

    /// Get the first path segment
    pub fn first_segment(&self) -> Option<&str> {
        self.segments().first().copied()
    }

    /// Get the last path segment
    pub fn last_segment(&self) -> Option<&str> {
        self.segments().last().copied()
    }
}

/// Decode percent-encoded characters in URL path
///
/// # Arguments
/// * `path` - URL path to decode
///
/// # Returns
/// * Decoded string, or original string if decoding fails
///
/// # Examples
/// ```
/// use ws_rs::path_utils::decode_url_path;
///
/// assert_eq!(decode_url_path("/ocpp/RDAM%7C123"), "/ocpp/RDAM|123");
/// assert_eq!(decode_url_path("/api/v1%2Fwebsocket"), "/api/v1/websocket");
/// assert_eq!(decode_url_path("/normal/path"), "/normal/path");
/// ```
pub fn decode_url_path(path: &str) -> String {
    match percent_decode_str(path).decode_utf8() {
        Ok(decoded) => {
            let result = decoded.to_string();
            if result != path {
                debug!("URL path decoded: '{}' -> '{}'", path, result);
            }
            result
        }
        Err(e) => {
            warn!("Failed to decode URL path '{}': {}", path, e);
            path.to_string()
        }
    }
}

/// Extract suffix from URL path (remove leading '/')
///
/// # Arguments
/// * `path` - URL path
///
/// # Returns
/// * Suffix string with leading '/' removed
///
/// # Examples
/// ```
/// use ws_rs::path_utils::extract_suffix;
///
/// assert_eq!(extract_suffix("/ocpp/CS001"), "ocpp/CS001");
/// assert_eq!(extract_suffix("/"), "");
/// assert_eq!(extract_suffix(""), "");
/// assert_eq!(extract_suffix("ocpp/CS001"), "ocpp/CS001");
/// ```
pub fn extract_suffix(path: &str) -> String {
    if path.starts_with('/') {
        path[1..].to_string()
    } else {
        path.to_string()
    }
}

/// Parse OCPP path and extract charging station ID
///
/// # Arguments
/// * `path_info` - Path information
///
/// # Returns
/// * If it's an OCPP path, returns the charging station ID; otherwise returns None
///
/// # Examples
/// ```
/// use ws_rs::path_utils::{PathInfo, parse_ocpp_station_id};
///
/// let path = PathInfo::new("/ocpp/CS001".to_string());
/// assert_eq!(parse_ocpp_station_id(&path), Some("CS001"));
///
/// let path = PathInfo::new("/ocpp/RDAM%7C123".to_string());
/// assert_eq!(parse_ocpp_station_id(&path), Some("RDAM|123"));
///
/// let path = PathInfo::new("/api/websocket".to_string());
/// assert_eq!(parse_ocpp_station_id(&path), None);
/// ```
pub fn parse_ocpp_station_id(path_info: &PathInfo) -> Option<&str> {
    if path_info.starts_with("ocpp/") {
        let segments = path_info.segments();
        if segments.len() >= 2 && segments[0] == "ocpp" {
            Some(segments[1])
        } else {
            None
        }
    } else {
        None
    }
}

/// Parse API path and extract version and endpoint information
///
/// # Arguments
/// * `path_info` - Path information
///
/// # Returns
/// * If it's an API path, returns (version, endpoint); otherwise returns None
///
/// # Examples
/// ```
/// use ws_rs::path_utils::{PathInfo, parse_api_path};
///
/// let path = PathInfo::new("/api/v1/websocket".to_string());
/// assert_eq!(parse_api_path(&path), Some(("v1", "websocket")));
///
/// let path = PathInfo::new("/api/v2/notifications".to_string());
/// assert_eq!(parse_api_path(&path), Some(("v2", "notifications")));
///
/// let path = PathInfo::new("/ocpp/CS001".to_string());
/// assert_eq!(parse_api_path(&path), None);
/// ```
pub fn parse_api_path(path_info: &PathInfo) -> Option<(&str, &str)> {
    if path_info.starts_with("api/") {
        let segments = path_info.segments();
        if segments.len() >= 3 && segments[0] == "api" {
            Some((segments[1], segments[2]))
        } else {
            None
        }
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_url_path() {
        assert_eq!(decode_url_path("/ocpp/CS001"), "/ocpp/CS001");
        assert_eq!(decode_url_path("/ocpp/RDAM%7C123"), "/ocpp/RDAM|123");
        assert_eq!(decode_url_path("/api/v1%2Fwebsocket"), "/api/v1/websocket");
        assert_eq!(decode_url_path("/path%20with%20spaces"), "/path with spaces");
        assert_eq!(decode_url_path(""), "");
    }

    #[test]
    fn test_extract_suffix() {
        assert_eq!(extract_suffix("/ocpp/CS001"), "ocpp/CS001");
        assert_eq!(extract_suffix("/"), "");
        assert_eq!(extract_suffix(""), "");
        assert_eq!(extract_suffix("ocpp/CS001"), "ocpp/CS001");
        assert_eq!(extract_suffix("/api/v1/websocket"), "api/v1/websocket");
    }

    #[test]
    fn test_path_info() {
        let path = PathInfo::new("/ocpp/RDAM%7C123".to_string());
        assert_eq!(path.raw(), "/ocpp/RDAM%7C123");
        assert_eq!(path.decoded(), "/ocpp/RDAM|123");
        assert_eq!(path.suffix(), "ocpp/RDAM%7C123");
        assert_eq!(path.decoded_suffix(), "ocpp/RDAM|123");
        assert!(!path.is_root());
        assert!(path.starts_with("ocpp/"));
        
        let segments = path.segments();
        assert_eq!(segments, vec!["ocpp", "RDAM|123"]);
        assert_eq!(path.first_segment(), Some("ocpp"));
        assert_eq!(path.last_segment(), Some("RDAM|123"));
    }

    #[test]
    fn test_root_path() {
        let path = PathInfo::new("/".to_string());
        assert!(path.is_root());
        assert_eq!(path.segments(), Vec::<&str>::new());
        assert_eq!(path.first_segment(), None);
        assert_eq!(path.last_segment(), None);
    }

    #[test]
    fn test_parse_ocpp_station_id() {
        let path1 = PathInfo::new("/ocpp/CS001".to_string());
        assert_eq!(parse_ocpp_station_id(&path1), Some("CS001"));
        
        let path2 = PathInfo::new("/ocpp/RDAM%7C123".to_string());
        assert_eq!(parse_ocpp_station_id(&path2), Some("RDAM|123"));
        
        let path3 = PathInfo::new("/api/websocket".to_string());
        assert_eq!(parse_ocpp_station_id(&path3), None);
    }

    #[test]
    fn test_parse_api_path() {
        let path1 = PathInfo::new("/api/v1/websocket".to_string());
        assert_eq!(parse_api_path(&path1), Some(("v1", "websocket")));

        let path2 = PathInfo::new("/api/v2/notifications".to_string());
        assert_eq!(parse_api_path(&path2), Some(("v2", "notifications")));

        let path3 = PathInfo::new("/ocpp/CS001".to_string());
        assert_eq!(parse_api_path(&path3), None);
    }

    #[test]
    fn test_comprehensive_url_conversion() {
        println!("\n🧪 Comprehensive URL conversion test:");

        // OCPP test cases
        let ocpp_tests = vec![
            ("/ocpp/CS001", "CS001"),
            ("/ocpp/RDAM%7C123", "RDAM|123"),
            ("/ocpp/Station%20Name%20With%20Spaces", "Station Name With Spaces"),
            ("/ocpp/Test%2BStation", "Test+Station"),
        ];

        for (raw_path, expected_id) in ocpp_tests {
            let path_info = PathInfo::new(raw_path.to_string());
            let station_id = parse_ocpp_station_id(&path_info).unwrap();
            assert_eq!(station_id, expected_id);
            println!("  ✅ OCPP: '{}' → ID: '{}'", raw_path, station_id);
        }

        // API test cases
        let api_tests = vec![
            ("/api/v1/websocket", ("v1", "websocket")),
            ("/api/v2/notifications", ("v2", "notifications")),
            ("/api/v3/data", ("v3", "data")),
        ];

        for (raw_path, (expected_version, expected_endpoint)) in api_tests {
            let path_info = PathInfo::new(raw_path.to_string());
            let (version, endpoint) = parse_api_path(&path_info).unwrap();
            assert_eq!(version, expected_version);
            assert_eq!(endpoint, expected_endpoint);
            println!("  ✅ API: '{}' → v{}, endpoint: '{}'", raw_path, version, endpoint);
        }

        // Custom path tests
        let custom_tests = vec![
            ("/custom/path%2Fwith%2Fslashes", vec!["custom", "path", "with", "slashes"]),
            ("/device/sensor%20data", vec!["device", "sensor data"]),
        ];

        for (raw_path, expected_segments) in custom_tests {
            let path_info = PathInfo::new(raw_path.to_string());
            let segments = path_info.segments();
            assert_eq!(segments, expected_segments);
            println!("  ✅ Custom: '{}' → segments: {:?}", raw_path, segments);
        }

        println!("  🎉 All URL conversion tests passed!");
    }
}

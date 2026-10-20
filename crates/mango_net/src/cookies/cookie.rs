//! Individual Cookie representation, validation, and profile serialization.

use std::time::{SystemTime, UNIX_EPOCH};

use super::same_site::{CookiePriority, SameSite};

/// A single HTTP cookie stored in the cookie jar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cookie {
    /// Cookie name.
    pub name: String,
    /// Cookie value.
    pub value: String,
    /// Domain for which the cookie is valid (normalized to lowercase without leading dot).
    pub domain: String,
    /// Path for which the cookie is valid.
    pub path: String,
    /// True if cookie requires secure (HTTPS) transport.
    pub secure: bool,
    /// True if cookie is inaccessible to client-side scripts.
    pub http_only: bool,
    /// Unix timestamp (seconds) at which the cookie expires.
    /// `None` means a session cookie that dies with the browser process.
    pub expires_at: Option<u64>,
    /// SameSite restriction level.
    pub same_site: SameSite,
    /// True when the cookie was set without an explicit `Domain` attribute and
    /// therefore only matches the exact origin host (RFC 6265 §5.3 step 6).
    pub host_only: bool,
    /// Creation timestamp (seconds) for RFC 6265 §5.4 sorting.
    pub creation_time: u64,
    /// Last access timestamp (seconds) for eviction.
    pub last_access_time: u64,
    /// Partition key for CHIPS (Cookies Having Independent Partitioned State).
    /// `None` indicates a standard unpartitioned cookie.
    pub partition_key: Option<String>,
    /// Priority level for Chromium-standard eviction.
    pub priority: CookiePriority,
}

impl Cookie {
    /// Creates a session cookie with default attributes.
    pub fn new(name: &str, value: &str, domain: &str, path: &str) -> Self {
        let now = now_secs();
        Cookie {
            name: name.to_string(),
            value: value.to_string(),
            domain: domain.trim_start_matches('.').to_ascii_lowercase(),
            path: if path.is_empty() {
                "/".to_string()
            } else {
                path.to_string()
            },
            secure: false,
            http_only: false,
            expires_at: None,
            same_site: SameSite::Unspecified,
            host_only: false,
            creation_time: now,
            last_access_time: now,
            partition_key: None,
            priority: CookiePriority::Medium,
        }
    }

    /// True when the cookie's expiry time has passed.
    pub fn is_expired(&self) -> bool {
        match self.expires_at {
            Some(t) => now_secs() >= t,
            None => false,
        }
    }

    /// True when this cookie may be exposed through `document.cookie`.
    /// HttpOnly cookies are reserved for network requests only.
    pub fn is_script_visible(&self) -> bool {
        !self.http_only && !self.is_expired()
    }

    /// Serializes the cookie for the profile file (one line, tab separated).
    pub fn to_profile_line(&self) -> String {
        format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            self.domain,
            encode_field(&self.path),
            u8::from(self.secure),
            u8::from(self.http_only),
            self.expires_at.map(|v| v.to_string()).unwrap_or_default(),
            u8::from(self.host_only),
            self.same_site.as_str(),
            encode_field(&self.name),
            encode_field(&self.value),
            self.creation_time,
            self.last_access_time,
            encode_field(self.partition_key.as_deref().unwrap_or("")),
            self.priority.as_str(),
        )
    }

    /// Parses a line produced by [`Cookie::to_profile_line`].
    pub fn from_profile_line(line: &str) -> Option<Cookie> {
        if line.trim().is_empty() || line.starts_with('#') {
            return None;
        }
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() < 9 {
            return None;
        }
        let expires_at = if fields[4].is_empty() {
            None
        } else {
            fields[4].parse::<u64>().ok()
        };
        let creation_time = if fields.len() > 9 {
            fields[9].parse::<u64>().unwrap_or(0)
        } else {
            0
        };
        let last_access_time = if fields.len() > 10 {
            fields[10].parse::<u64>().unwrap_or(0)
        } else {
            creation_time
        };
        let partition_key = if fields.len() > 11 && !fields[11].is_empty() {
            Some(decode_field(fields[11]))
        } else {
            None
        };
        let priority = if fields.len() > 12 {
            CookiePriority::from_str(fields[12])
        } else {
            CookiePriority::Medium
        };
        let cookie = Cookie {
            domain: fields[0].trim_start_matches('.').to_ascii_lowercase(),
            path: decode_field(fields[1]),
            secure: fields[2] == "1",
            http_only: fields[3] == "1",
            expires_at,
            host_only: fields[5] == "1",
            same_site: SameSite::from_str(fields[6]),
            name: decode_field(fields[7]),
            value: decode_field(fields[8]),
            creation_time,
            last_access_time,
            partition_key,
            priority,
        };
        if cookie.name.is_empty() || cookie.domain.is_empty() {
            return None;
        }
        Some(cookie)
    }
}

/// Current wall-clock time as a Unix timestamp in seconds.
pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Percent-encodes tab, newline, and carriage return so that a cookie value can
/// never break the one-cookie-per-line profile format.
pub fn encode_field(s: &str) -> String {
    s.replace('%', "%25")
        .replace('\t', "%09")
        .replace('\n', "%0A")
        .replace('\r', "%0D")
}

/// Decodes percent-encoded fields from the profile format.
pub fn decode_field(s: &str) -> String {
    s.replace("%0D", "\r")
        .replace("%0A", "\n")
        .replace("%09", "\t")
        .replace("%25", "%")
}

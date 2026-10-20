//! Certificate Pinning (HPKP / SPKI Pinning) and Network Proxy configuration.

use std::collections::HashMap;

/// Certificate Pinning management and verification.
#[derive(Debug, Clone, Default)]
pub struct CertificatePinSet {
    /// Maps normalized hostname (e.g., "api.example.com") to set of allowed base64-encoded SPKI SHA-256 pin hashes.
    pub host_pins: HashMap<String, Vec<String>>,
    /// Include subdomains flags per host.
    pub include_subdomains: HashMap<String, bool>,
}

/// Errors occurring during certificate pin verification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PinError {
    PinMismatch {
        host: String,
        presented: Vec<String>,
        expected: Vec<String>,
    },
}

impl std::fmt::Display for PinError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PinError::PinMismatch {
                host,
                presented,
                expected,
            } => {
                write!(
                    f,
                    "Certificate pinning mismatch for {host}: presented {presented:?}, expected {expected:?}"
                )
            }
        }
    }
}

impl std::error::Error for PinError {}

impl CertificatePinSet {
    /// Creates an empty certificate pin set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds an SPKI SHA-256 base64 pin hash for a host.
    pub fn add_pin(
        &mut self,
        host: &str,
        pin_hash_base64: impl Into<String>,
        include_subdomains: bool,
    ) {
        let h = host.trim().to_ascii_lowercase();
        self.host_pins
            .entry(h.clone())
            .or_default()
            .push(pin_hash_base64.into());
        if include_subdomains {
            self.include_subdomains.insert(h, true);
        }
    }

    /// Verifies presented SPKI pin hashes against configured pins for `host`.
    pub fn verify_pins(
        &self,
        host: &str,
        presented_spki_hashes: &[String],
    ) -> Result<bool, PinError> {
        let host_lower = host.trim().to_ascii_lowercase();
        // Check exact match or ancestor subdomain match
        let mut matching_host = None;
        if self.host_pins.contains_key(&host_lower) {
            matching_host = Some(host_lower.clone());
        } else {
            let parts: Vec<&str> = host_lower.split('.').collect();
            for i in 1..parts.len() {
                let ancestor = parts[i..].join(".");
                if let Some(true) = self.include_subdomains.get(&ancestor) {
                    matching_host = Some(ancestor);
                    break;
                }
            }
        }

        let Some(target_host) = matching_host else {
            return Ok(true); // No pins configured for this host
        };

        let expected_pins = self.host_pins.get(&target_host).unwrap();
        for presented in presented_spki_hashes {
            if expected_pins.iter().any(|exp| exp == presented) {
                return Ok(true);
            }
        }

        Err(PinError::PinMismatch {
            host: host.to_string(),
            presented: presented_spki_hashes.to_vec(),
            expected: expected_pins.clone(),
        })
    }
}

/// Network proxy configuration supporting HTTP/HTTPS/SOCKS5 proxies and bypass rules.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProxyConfig {
    pub http_proxy: Option<String>,
    pub https_proxy: Option<String>,
    pub all_proxy: Option<String>,
    pub no_proxy: Vec<String>,
}

impl ProxyConfig {
    /// Loads proxy configuration from standard environment variables (HTTP_PROXY, HTTPS_PROXY, ALL_PROXY, NO_PROXY).
    pub fn from_env() -> Self {
        let http_proxy = std::env::var("HTTP_PROXY")
            .or_else(|_| std::env::var("http_proxy"))
            .ok();
        let https_proxy = std::env::var("HTTPS_PROXY")
            .or_else(|_| std::env::var("https_proxy"))
            .ok();
        let all_proxy = std::env::var("ALL_PROXY")
            .or_else(|_| std::env::var("all_proxy"))
            .ok();
        let no_proxy_str = std::env::var("NO_PROXY")
            .or_else(|_| std::env::var("no_proxy"))
            .unwrap_or_default();
        let no_proxy = no_proxy_str
            .split(',')
            .map(|s| s.trim().to_ascii_lowercase())
            .filter(|s| !s.is_empty())
            .collect();
        Self {
            http_proxy,
            https_proxy,
            all_proxy,
            no_proxy,
        }
    }

    /// Returns `true` if requests to `host` should bypass configured proxies.
    pub fn should_bypass(&self, host: &str) -> bool {
        let h = host.trim().to_ascii_lowercase();
        if h == "localhost" || h == "127.0.0.1" || h == "::1" {
            return true;
        }
        for pattern in &self.no_proxy {
            if pattern == "*" {
                return true;
            }
            if pattern.starts_with('.') {
                if h.ends_with(pattern) || h == &pattern[1..] {
                    return true;
                }
            } else if h == *pattern || h.ends_with(&format!(".{pattern}")) {
                return true;
            }
        }
        false
    }

    /// Resolves the proxy URL to use for the given request URL, or `None` if direct connection.
    pub fn resolve_proxy_for_url(&self, url: &crate::url::Url) -> Option<String> {
        let host = url.host.as_deref()?;
        if self.should_bypass(host) {
            return None;
        }
        let scheme = url.scheme.as_str().to_ascii_lowercase();
        if scheme == "https" {
            self.https_proxy.clone().or_else(|| self.all_proxy.clone())
        } else if scheme == "http" {
            self.http_proxy.clone().or_else(|| self.all_proxy.clone())
        } else {
            self.all_proxy.clone()
        }
    }
}

//! SameSite and CookiePriority attribute types for RFC 6265bis cookies.

/// The `SameSite` attribute of a cookie (RFC 6265bis §5.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SameSite {
    /// Attribute absent — browsers treat this as `Lax` for top-level requests.
    #[default]
    Unspecified,
    /// Sent only for same-site requests.
    Strict,
    /// Sent for same-site requests and top-level cross-site navigations.
    Lax,
    /// Sent with every request that matches domain/path.
    None,
}

impl SameSite {
    /// Returns the lowercase string representation of this SameSite value.
    pub fn as_str(self) -> &'static str {
        match self {
            SameSite::Unspecified => "unspecified",
            SameSite::Strict => "strict",
            SameSite::Lax => "lax",
            SameSite::None => "none",
        }
    }

    /// Parses a SameSite attribute string value.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        match s.trim().trim_matches('"').to_ascii_lowercase().as_str() {
            "strict" => SameSite::Strict,
            "lax" => SameSite::Lax,
            "none" => SameSite::None,
            _ => SameSite::Unspecified,
        }
    }
}

/// Cookie Priority attribute (Chromium extension / RFC 6265bis Draft).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum CookiePriority {
    /// Low priority eviction tier.
    Low = 1,
    /// Default medium priority.
    #[default]
    Medium = 2,
    /// High priority eviction tier (evicted last).
    High = 3,
}

impl CookiePriority {
    /// Returns the lowercase string representation of this Priority value.
    pub fn as_str(self) -> &'static str {
        match self {
            CookiePriority::Low => "low",
            CookiePriority::Medium => "medium",
            CookiePriority::High => "high",
        }
    }

    /// Parses a Priority attribute string value.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        match s.trim().trim_matches('"').to_ascii_lowercase().as_str() {
            "low" => CookiePriority::Low,
            "high" => CookiePriority::High,
            _ => CookiePriority::Medium,
        }
    }
}

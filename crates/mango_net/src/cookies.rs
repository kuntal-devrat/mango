//! RFC 6265 cookie storage with disk persistence.
//!
//! This module owns the browser's cookie jar. Cookies are stored per
//! (name, domain, path) and are persisted to a profile file so that sessions
//! survive a browser restart (PRD GAP-016). The serialization format is a
//! line-oriented, tab-separated text file (Netscape `cookies.txt` compatible
//! in shape) which keeps the profile human-readable and diffable.

use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::url::Url;

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
    fn as_str(self) -> &'static str {
        match self {
            SameSite::Unspecified => "unspecified",
            SameSite::Strict => "strict",
            SameSite::Lax => "lax",
            SameSite::None => "none",
        }
    }

    fn from_str(s: &str) -> Self {
        match s.trim().trim_matches('"').to_ascii_lowercase().as_str() {
            "strict" => SameSite::Strict,
            "lax" => SameSite::Lax,
            "none" => SameSite::None,
            _ => SameSite::Unspecified,
        }
    }
}

/// A single HTTP cookie stored in the cookie jar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cookie {
    pub name: String,
    pub value: String,
    pub domain: String,
    pub path: String,
    pub secure: bool,
    pub http_only: bool,
    /// Unix timestamp (seconds) at which the cookie expires.
    /// `None` means a session cookie that dies with the browser process.
    pub expires_at: Option<u64>,
    pub same_site: SameSite,
    /// True when the cookie was set without an explicit `Domain` attribute and
    /// therefore only matches the exact origin host (RFC 6265 §5.3 step 6).
    pub host_only: bool,
    /// Creation timestamp (seconds) for RFC 6265 §5.4 sorting.
    pub creation_time: u64,
    /// Last access timestamp (seconds) for eviction.
    pub last_access_time: u64,
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
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
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
fn encode_field(s: &str) -> String {
    s.replace('%', "%25")
        .replace('\t', "%09")
        .replace('\n', "%0A")
        .replace('\r', "%0D")
}

fn decode_field(s: &str) -> String {
    s.replace("%0D", "\r")
        .replace("%0A", "\n")
        .replace("%09", "\t")
        .replace("%25", "%")
}

/// Returns true if host represents an IPv4 or IPv6 address.
pub fn is_ip_address(host: &str) -> bool {
    host.parse::<IpAddr>().is_ok()
}

/// Returns true if `domain` is a known public suffix (top-level or two-level domain).
/// RFC 6265 §5.3 step 5 forbids setting cookies for public suffixes.
pub fn is_public_suffix(domain: &str) -> bool {
    let d = domain.trim_start_matches('.').to_ascii_lowercase();
    if d.is_empty() {
        return false;
    }
    if !d.contains('.') {
        return d != "localhost";
    }

    const MULTI_PART_SUFFIXES: &[&str] = &[
        "co.uk",
        "org.uk",
        "gov.uk",
        "ac.uk",
        "net.uk",
        "me.uk",
        "ltd.uk",
        "plc.uk",
        "com.au",
        "net.au",
        "org.au",
        "edu.au",
        "gov.au",
        "asn.au",
        "id.au",
        "co.jp",
        "ne.jp",
        "or.jp",
        "ac.jp",
        "go.jp",
        "ed.jp",
        "co.nz",
        "org.nz",
        "net.nz",
        "govt.nz",
        "ac.nz",
        "geek.nz",
        "com.br",
        "org.br",
        "net.br",
        "gov.br",
        "edu.br",
        "com.cn",
        "net.cn",
        "org.cn",
        "gov.cn",
        "edu.cn",
        "co.in",
        "net.in",
        "org.in",
        "gen.in",
        "firm.in",
        "ind.in",
        "nic.in",
        "ac.in",
        "edu.in",
        "res.in",
        "gov.in",
        "mil.in",
        "co.za",
        "org.za",
        "gov.za",
        "ac.za",
        "net.za",
        "com.sg",
        "edu.sg",
        "gov.sg",
        "net.sg",
        "org.sg",
        "com.mx",
        "org.mx",
        "gob.mx",
        "edu.mx",
        "net.mx",
        "co.kr",
        "ne.kr",
        "or.kr",
        "re.kr",
        "pe.kr",
        "go.kr",
        "ac.kr",
        "com.tw",
        "org.tw",
        "net.tw",
        "gov.tw",
        "edu.tw",
        "idv.tw",
        "com.hk",
        "org.hk",
        "net.hk",
        "gov.hk",
        "edu.hk",
        "com.tr",
        "org.tr",
        "net.tr",
        "gov.tr",
        "edu.tr",
        "com.ua",
        "net.ua",
        "org.ua",
        "gov.ua",
        "edu.ua",
        "co.id",
        "or.id",
        "web.id",
        "go.id",
        "ac.id",
        "sch.id",
        "com.my",
        "org.my",
        "net.my",
        "gov.my",
        "edu.my",
        "com.ph",
        "org.ph",
        "net.ph",
        "gov.ph",
        "edu.ph",
        "com.vn",
        "net.vn",
        "org.vn",
        "gov.vn",
        "edu.vn",
        "com.ar",
        "org.ar",
        "gob.ar",
        "net.ar",
        "edu.ar",
        "co.il",
        "org.il",
        "gov.il",
        "ac.il",
        "muni.il",
        "github.io",
        "gitlab.io",
        "vercel.app",
        "netlify.app",
        "pages.dev",
    ];

    MULTI_PART_SUFFIXES.iter().any(|&s| d == s)
}

/// Returns the registrable domain (eTLD+1) for a hostname according to RFC 6265bis.
pub fn get_registrable_domain(host: &str) -> String {
    let d = host.trim().trim_start_matches('.').to_ascii_lowercase();
    if is_ip_address(&d) || d == "localhost" {
        return d;
    }
    let parts: Vec<&str> = d.split('.').collect();
    if parts.len() <= 2 {
        return d;
    }
    let last_two = format!("{}.{}", parts[parts.len() - 2], parts[parts.len() - 1]);
    if is_public_suffix(&last_two) && parts.len() >= 3 {
        return format!("{}.{}", parts[parts.len() - 3], last_two);
    }
    format!("{}.{}", parts[parts.len() - 2], parts[parts.len() - 1])
}

/// Checks if two hostnames belong to the same site (same registrable domain).
pub fn is_same_site_hosts(h1: &str, h2: &str) -> bool {
    let d1 = get_registrable_domain(h1);
    let d2 = get_registrable_domain(h2);
    d1 == d2
}

/// An RFC 6265 compliant cookie storage jar.
#[derive(Debug, Clone, Default)]
pub struct CookieJar {
    cookies: Vec<Cookie>,
}

impl CookieJar {
    pub fn new() -> Self {
        Self {
            cookies: Vec::new(),
        }
    }

    /// Parses a `Set-Cookie` response header value for a given request URL.
    ///
    /// Validates attributes: Max-Age, Expires, Domain, Path, Secure, HttpOnly, SameSite,
    /// and enforces RFC 6265 / RFC 6265bis domain-matching, public suffix restrictions,
    /// SameSite=None requirements, and `__Secure-` / `__Host-` prefixes.
    pub fn parse_set_cookie(header_value: &str, request_url: &Url) -> Option<Cookie> {
        let request_host = match &request_url.host {
            Some(h) if !h.is_empty() => h.to_ascii_lowercase(),
            _ => return None,
        };
        let is_https = request_url.scheme.eq_ignore_ascii_case("https");

        let mut parts = header_value.split(';');
        let name_value = parts.next()?.trim();
        let (name, value) = name_value.split_once('=')?;
        let name = name.trim().to_string();
        let mut value = value.trim().to_string();
        if name.is_empty() {
            return None;
        }

        // Unquote value if wrapped in double quotes (RFC 6265 §4.1.1)
        if value.starts_with('"') && value.ends_with('"') && value.len() >= 2 {
            value = value[1..value.len() - 1].to_string();
        }

        let mut domain = request_host.clone();
        let mut explicit_domain = false;
        let mut path = default_cookie_path(&request_url.path);
        let mut secure = false;
        let mut http_only = false;
        let mut is_deleted = false;
        let mut expires_at = None;
        let mut same_site = SameSite::Unspecified;

        for part in parts {
            let trimmed = part.trim();
            if trimmed.is_empty() {
                continue;
            }
            if let Some((attr_name, attr_val)) = trimmed.split_once('=') {
                let attr_lower = attr_name.trim().to_ascii_lowercase();
                let val = attr_val.trim().trim_matches('"');
                match attr_lower.as_str() {
                    "domain" => {
                        let clean_domain = val.trim_start_matches('.').to_ascii_lowercase();
                        if !clean_domain.is_empty() {
                            // RFC 6265 §5.3 step 5:
                            // Check that request_host domain-matches clean_domain
                            if is_ip_address(&request_host) {
                                if clean_domain != request_host {
                                    return None;
                                }
                            } else {
                                let matches = request_host == clean_domain
                                    || request_host.ends_with(&format!(".{clean_domain}"));
                                if !matches {
                                    return None;
                                }
                                // Public suffix check: cannot set cookie for public suffix
                                if is_public_suffix(&clean_domain) {
                                    return None;
                                }
                            }
                            domain = clean_domain;
                            explicit_domain = true;
                        }
                    }
                    "path" => {
                        if val.starts_with('/') {
                            path = val.to_string();
                        }
                    }
                    "max-age" => {
                        let clean_num = val.trim_start_matches('+');
                        match clean_num.parse::<i64>() {
                            Ok(secs) if secs <= 0 => {
                                is_deleted = true;
                                expires_at = Some(0);
                            }
                            Ok(secs) => expires_at = Some(now_secs() + secs as u64),
                            Err(_) => {}
                        }
                    }
                    "expires" => {
                        // `Max-Age` wins over `Expires` (RFC 6265 §5.3 step 3),
                        // so only apply this when Max-Age was not seen.
                        if expires_at.is_none()
                            && let Some(ts) = parse_http_date(val)
                        {
                            if ts <= now_secs() {
                                is_deleted = true;
                            }
                            expires_at = Some(ts);
                        }
                    }
                    "samesite" => same_site = SameSite::from_str(val),
                    "secure" => secure = true,
                    "httponly" => http_only = true,
                    _ => {}
                }
            } else {
                let attr_lower = trimmed.to_ascii_lowercase();
                match attr_lower.as_str() {
                    "secure" => secure = true,
                    "httponly" => http_only = true,
                    _ => {}
                }
            }
        }

        // RFC 6265bis §5.4: Secure cookies must only be accepted over secure transport (HTTPS)
        if secure && !is_https {
            return None;
        }

        // RFC 6265bis: SameSite=None requires Secure
        if same_site == SameSite::None && !secure {
            return None;
        }

        // RFC 6265bis Cookie Prefixes:
        // __Secure- prefix requires Secure and HTTPS
        if name.starts_with("__Secure-") && (!secure || !is_https) {
            return None;
        }

        // __Host- prefix requires Secure, HTTPS, Path=/, and NO explicit Domain attribute
        if name.starts_with("__Host-") && (!secure || !is_https || explicit_domain || path != "/") {
            return None;
        }

        let host_only = !explicit_domain;
        let now = now_secs();

        if is_deleted {
            return Some(Cookie {
                name,
                value: String::new(),
                domain,
                path,
                secure,
                http_only,
                expires_at: Some(0),
                same_site,
                host_only,
                creation_time: now,
                last_access_time: now,
            });
        }

        Some(Cookie {
            name,
            value,
            domain,
            path,
            secure,
            http_only,
            expires_at,
            same_site,
            host_only,
            creation_time: now,
            last_access_time: now,
        })
    }

    /// Stores or deletes a cookie in the jar.
    pub fn store_cookie(&mut self, mut cookie: Cookie) {
        if cookie.value.is_empty() || cookie.is_expired() {
            self.remove_matching(&cookie);
            return;
        }

        let now = now_secs();
        if let Some(existing) = self.cookies.iter_mut().find(|c| {
            c.name.eq_ignore_ascii_case(&cookie.name)
                && c.domain.eq_ignore_ascii_case(&cookie.domain)
                && c.path == cookie.path
        }) {
            // RFC 6265 §5.3 step 11: preserve creation_time of existing cookie
            cookie.creation_time = existing.creation_time;
            cookie.last_access_time = now;
            *existing = cookie;
        } else {
            if cookie.creation_time == 0 {
                cookie.creation_time = now;
            }
            cookie.last_access_time = now;
            self.cookies.push(cookie);
        }

        self.enforce_limits();
    }

    fn enforce_limits(&mut self) {
        const MAX_TOTAL: usize = 3000;
        if self.cookies.len() <= MAX_TOTAL {
            return;
        }
        self.prune_expired();
        if self.cookies.len() > MAX_TOTAL {
            self.cookies.sort_by_key(|c| c.last_access_time);
            let drop_count = self.cookies.len() - MAX_TOTAL;
            self.cookies.drain(0..drop_count);
        }
    }

    fn remove_matching(&mut self, cookie: &Cookie) {
        self.cookies.retain(|c| {
            !(c.name.eq_ignore_ascii_case(&cookie.name)
                && c.domain.eq_ignore_ascii_case(&cookie.domain)
                && c.path == cookie.path)
        });
    }

    /// Removes every cookie for a domain (used by "clear site data").
    pub fn remove_for_domain(&mut self, domain: &str) {
        let dom = domain.trim_start_matches('.').to_ascii_lowercase();
        self.cookies.retain(|c| c.domain != dom);
    }

    /// Drops session cookies (cookies without a persistent expiry time).
    pub fn clear_session_cookies(&mut self) -> usize {
        let before = self.cookies.len();
        self.cookies.retain(|c| c.expires_at.is_some());
        before - self.cookies.len()
    }

    /// Drops cookies whose expiry time has passed.
    pub fn prune_expired(&mut self) -> usize {
        let before = self.cookies.len();
        self.cookies.retain(|c| !c.is_expired());
        before - self.cookies.len()
    }

    /// Returns all cookies matching the given URL and same-site context, longest-path first.
    pub fn get_cookies_with_context(
        &self,
        url: &Url,
        initiator_url: Option<&Url>,
        is_top_level_navigation: bool,
        is_safe_method: bool,
    ) -> Vec<Cookie> {
        let host = match &url.host {
            Some(h) => h.to_ascii_lowercase(),
            None => return Vec::new(),
        };
        let is_https = url.scheme.eq_ignore_ascii_case("https");
        let path = if url.path.is_empty() { "/" } else { &url.path };

        let is_same_site = match (
            initiator_url.and_then(|u| u.host.as_deref()),
            url.host.as_deref(),
        ) {
            (Some(h1), Some(h2)) => is_same_site_hosts(h1, h2),
            _ => true,
        };

        let mut matching: Vec<Cookie> = self
            .cookies
            .iter()
            .filter(|c| {
                if c.is_expired() {
                    return false;
                }
                if c.secure && !is_https {
                    return false;
                }
                if !cookie_domain_matches(&host, &c.domain, c.host_only) {
                    return false;
                }
                if !path_matches(path, &c.path) {
                    return false;
                }

                // RFC 6265bis SameSite enforcement:
                if !is_same_site {
                    match c.same_site {
                        SameSite::Strict => return false,
                        SameSite::Lax => {
                            if !is_top_level_navigation || !is_safe_method {
                                return false;
                            }
                        }
                        SameSite::None => {}
                        SameSite::Unspecified => {
                            // Default behavior treats unspecified as Lax for safety
                            if !is_top_level_navigation || !is_safe_method {
                                return false;
                            }
                        }
                    }
                }

                true
            })
            .cloned()
            .collect();

        // RFC 6265 §5.4: longer paths sort first; ties broken by earlier creation-time.
        matching.sort_by(|a, b| {
            b.path
                .len()
                .cmp(&a.path.len())
                .then_with(|| a.creation_time.cmp(&b.creation_time))
        });
        matching
    }

    /// Returns all cookies matching the given URL, longest-path first,
    /// with equal-length ties broken by earlier creation-time (RFC 6265 §5.4).
    pub fn get_cookies(&self, url: &Url) -> Vec<Cookie> {
        self.get_cookies_with_context(url, None, true, true)
    }

    /// Generates the `Cookie: name=value; name2=value2` header string for a URL and same-site context.
    pub fn get_cookie_header_with_context(
        &self,
        url: &Url,
        initiator_url: Option<&Url>,
        is_top_level_navigation: bool,
        is_safe_method: bool,
    ) -> Option<String> {
        let matching = self.get_cookies_with_context(
            url,
            initiator_url,
            is_top_level_navigation,
            is_safe_method,
        );
        if matching.is_empty() {
            return None;
        }
        let pairs: Vec<String> = matching
            .into_iter()
            .map(|c| format!("{}={}", c.name, c.value))
            .collect();
        Some(pairs.join("; "))
    }

    /// Generates the `Cookie: name=value; name2=value2` header string for a URL.
    pub fn get_cookie_header(&self, url: &Url) -> Option<String> {
        self.get_cookie_header_with_context(url, None, true, true)
    }

    /// Builds the `document.cookie` string for a URL.
    ///
    /// HttpOnly cookies and expired cookies are never exposed to scripts.
    pub fn script_cookie_string(&self, url: &Url) -> String {
        let visible: Vec<String> = self
            .get_cookies(url)
            .into_iter()
            .filter(|c| c.is_script_visible())
            .map(|c| format!("{}={}", c.name, c.value))
            .collect();
        visible.join("; ")
    }

    /// Applies a `document.cookie` assignment string for a URL.
    ///
    /// `path`, `domain`, `max-age`, `expires`, `samesite`, and `secure` are
    /// honored; `HttpOnly` is deliberately ignored for script-set cookies so a
    /// script cannot take ownership of a server-managed cookie.
    pub fn set_from_script(&mut self, cookie_string: &str, url: &Url) -> bool {
        let Some(mut cookie) = CookieJar::parse_set_cookie(cookie_string, url) else {
            return false;
        };
        if cookie.name.is_empty() {
            return false;
        }
        cookie.http_only = false;
        self.store_cookie(cookie);
        true
    }

    /// Returns a snapshot of every cookie currently held.
    pub fn all(&self) -> Vec<Cookie> {
        self.cookies.clone()
    }

    /// Number of cookies currently held (including expired-but-unpruned ones).
    pub fn len(&self) -> usize {
        self.cookies.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cookies.is_empty()
    }

    /// Clears all cookies in the jar.
    pub fn clear(&mut self) {
        self.cookies.clear();
    }

    /// Serializes the jar into the profile file format.
    pub fn to_profile_string(&self) -> String {
        let mut out = String::from("# Mango cookie jar v1\n");
        out.push_str("# domain\tpath\tsecure\thttponly\texpires\thost_only\tsamesite\tname\tvalue\tcreation_time\tlast_access_time\n");
        for cookie in &self.cookies {
            if cookie.is_expired() {
                continue;
            }
            out.push_str(&cookie.to_profile_line());
            out.push('\n');
        }
        out
    }

    /// Replaces the jar contents from a profile file body.
    pub fn load_profile_string(&mut self, body: &str) {
        self.cookies.clear();
        for line in body.lines() {
            if let Some(cookie) = Cookie::from_profile_line(line)
                && !cookie.is_expired()
            {
                self.store_cookie(cookie);
            }
        }
    }

    /// Loads cookies from a profile file. Missing files are not an error.
    pub fn load_from_file(&mut self, path: &Path) -> std::io::Result<usize> {
        match std::fs::read_to_string(path) {
            Ok(body) => {
                self.load_profile_string(&body);
                Ok(self.cookies.len())
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(0),
            Err(e) => Err(e),
        }
    }

    /// Writes the jar to a profile file atomically, creating parent directories as needed.
    pub fn save_to_file(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)?;
        }
        let tmp_path = path.with_extension("tmp");
        if std::fs::write(&tmp_path, self.to_profile_string()).is_ok() {
            if std::fs::rename(&tmp_path, path).is_ok() {
                return Ok(());
            }
            let _ = std::fs::remove_file(&tmp_path);
        }
        std::fs::write(path, self.to_profile_string())
    }

    /// The default profile cookie file (`<data dir>/cookies.txt`).
    ///
    /// Honors `MANGO_PROFILE_DIR`, then `XDG_DATA_HOME`, then `%APPDATA%` on
    /// Windows, then `$HOME/.mango`.
    pub fn default_profile_path() -> PathBuf {
        if let Ok(dir) = std::env::var("MANGO_PROFILE_DIR")
            && !dir.is_empty()
        {
            return PathBuf::from(dir).join("cookies.txt");
        }
        if let Ok(xdg) = std::env::var("XDG_DATA_HOME")
            && !xdg.is_empty()
        {
            return PathBuf::from(xdg).join("mango").join("cookies.txt");
        }
        if let Ok(appdata) = std::env::var("APPDATA")
            && !appdata.is_empty()
        {
            return PathBuf::from(appdata).join("Mango").join("cookies.txt");
        }
        if let Ok(home) = std::env::var("HOME")
            && !home.is_empty()
        {
            return PathBuf::from(home).join(".mango").join("cookies.txt");
        }
        PathBuf::from("mango_profile").join("cookies.txt")
    }
}

/// RFC 6265 §5.1.4 default path: everything up to (not including) the last `/`.
fn default_cookie_path(request_path: &str) -> String {
    if request_path.is_empty() || !request_path.starts_with('/') {
        return "/".to_string();
    }
    match request_path.rfind('/') {
        Some(0) => "/".to_string(),
        Some(idx) => request_path[..idx].to_string(),
        None => "/".to_string(),
    }
}

fn cookie_domain_matches(url_host: &str, cookie_domain: &str, host_only: bool) -> bool {
    let host = url_host.to_ascii_lowercase();
    let c_dom = cookie_domain.trim_start_matches('.').to_ascii_lowercase();
    if host == c_dom {
        return true;
    }
    if host_only {
        return false;
    }
    if is_ip_address(&host) {
        return false;
    }
    host.ends_with(&format!(".{c_dom}"))
}

fn path_matches(url_path: &str, cookie_path: &str) -> bool {
    let u_path = if url_path.is_empty() { "/" } else { url_path };
    let c_path = if cookie_path.is_empty() {
        "/"
    } else {
        cookie_path
    };
    if u_path == c_path {
        return true;
    }
    if u_path.starts_with(c_path)
        && (c_path.ends_with('/') || u_path[c_path.len()..].starts_with('/'))
    {
        return true;
    }
    false
}

const MONTHS: [&str; 12] = [
    "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
];

/// Parses the RFC 1123 / RFC 850 / asctime date formats used by `Expires`.
pub fn parse_http_date(input: &str) -> Option<u64> {
    let cleaned: String = input.replace([',', '-'], " ").chars().collect();
    let tokens: Vec<String> = cleaned
        .split_whitespace()
        .map(|t| t.to_ascii_lowercase())
        .collect();
    if tokens.len() < 3 {
        return None;
    }

    let mut day: Option<u32> = None;
    let mut month: Option<u32> = None;
    let mut year: Option<i64> = None;
    let mut time: Option<(u32, u32, u32)> = None;

    for token in &tokens {
        if time.is_none() && token.contains(':') {
            let mut parts = token.split(':');
            let h = parts.next().and_then(|v| v.parse::<u32>().ok());
            let m = parts.next().and_then(|v| v.parse::<u32>().ok());
            let s = parts
                .next()
                .and_then(|v| v.parse::<u32>().ok())
                .unwrap_or(0);
            if let (Some(h), Some(m)) = (h, m) {
                time = Some((h, m, s));
                continue;
            }
        }
        if month.is_none()
            && let Some(idx) = MONTHS.iter().position(|m| token.starts_with(m))
        {
            month = Some(idx as u32 + 1);
            continue;
        }
        if let Ok(num) = token.parse::<i64>() {
            if day.is_none() && (1..=31).contains(&num) {
                day = Some(num as u32);
            } else if year.is_none() {
                // RFC 6265 §5.1.1: 70..=99 → 19xx, 00..=69 → 20xx.
                year = Some(match num {
                    0..=69 => num + 2000,
                    70..=99 => num + 1900,
                    _ => num,
                });
            }
        }
    }

    let (day, month, year, time) = (day?, month?, year?, time?);
    if year < 1601 {
        return None;
    }
    Some(
        days_from_civil(year, month, day) * 86_400
            + time.0 as u64 * 3600
            + time.1 as u64 * 60
            + time.2 as u64,
    )
}

/// Howard Hinnant's days-from-civil algorithm (days since 1970-01-01).
fn days_from_civil(y: i64, m: u32, d: u32) -> u64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = (if y >= 0 { y } else { y - 399 }) / 400;
    let yoe = (y - era * 400) as u64;
    let mp = ((m + 9) % 12) as u64;
    let doy = (153 * mp + 2) / 5 + d as u64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe as i64 - 719_468;
    days.max(0) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(s: &str) -> Url {
        Url::parse(s).expect("url")
    }

    #[test]
    fn test_cookie_jar_storage_and_header_generation() {
        let mut jar = CookieJar::new();
        let u = url("https://duckduckgo.com/search?q=rust");

        let c1 = CookieJar::parse_set_cookie("session=abc123; Path=/; Secure; HttpOnly", &u)
            .expect("cookie 1");
        jar.store_cookie(c1);
        let c2 = CookieJar::parse_set_cookie("theme=dark; Path=/search; Max-Age=3600", &u)
            .expect("cookie 2");
        jar.store_cookie(c2);

        let header = jar.get_cookie_header(&u).expect("header");
        assert!(header.contains("session=abc123"));
        assert!(header.contains("theme=dark"));
        // http:// must not receive the Secure cookie.
        let plain = url("http://duckduckgo.com/search");
        let plain_header = jar.get_cookie_header(&plain).unwrap_or_default();
        assert!(!plain_header.contains("session=abc123"));
    }

    #[test]
    fn test_cookie_deletion_via_max_age_zero() {
        let mut jar = CookieJar::new();
        let u = url("https://duckduckgo.com/");
        jar.store_cookie(
            CookieJar::parse_set_cookie("p=1; Domain=duckduckgo.com; Path=/", &u).unwrap(),
        );
        assert_eq!(jar.len(), 1);
        jar.store_cookie(
            CookieJar::parse_set_cookie("p=deleted; Max-Age=0; Domain=duckduckgo.com; Path=/", &u)
                .unwrap(),
        );
        assert_eq!(jar.len(), 0);
    }

    #[test]
    fn test_default_path_follows_last_slash() {
        let u = url("https://example.com/a/b/c?x=1");
        let c = CookieJar::parse_set_cookie("k=v", &u).unwrap();
        assert_eq!(c.path, "/a/b");
        let root = url("https://example.com/");
        let rc = CookieJar::parse_set_cookie("k=v", &root).unwrap();
        assert_eq!(rc.path, "/");
    }

    #[test]
    fn test_host_only_cookie_is_not_sent_to_subdomains() {
        let mut jar = CookieJar::new();
        let host = url("https://example.com/");
        jar.store_cookie(CookieJar::parse_set_cookie("a=1", &host).unwrap());
        let sub = url("https://api.example.com/");
        assert!(jar.get_cookie_header(&sub).is_none());

        let with_domain = CookieJar::parse_set_cookie("b=2; Domain=.example.com", &host).unwrap();
        jar.store_cookie(with_domain);
        assert!(jar.get_cookie_header(&sub).is_some());
    }

    #[test]
    fn test_http_only_cookies_are_hidden_from_scripts() {
        let mut jar = CookieJar::new();
        let u = url("https://example.com/");
        jar.store_cookie(CookieJar::parse_set_cookie("secret=1; HttpOnly", &u).unwrap());
        jar.store_cookie(CookieJar::parse_set_cookie("open=2", &u).unwrap());
        let script_view = jar.script_cookie_string(&u);
        assert!(!script_view.contains("secret=1"));
        assert!(script_view.contains("open=2"));
    }

    #[test]
    fn test_document_cookie_assignment_roundtrip() {
        let mut jar = CookieJar::new();
        let u = url("https://example.com/app/page");
        assert!(jar.set_from_script("token=xyz; Path=/", &u));
        assert_eq!(jar.script_cookie_string(&u), "token=xyz");

        // HttpOnly must not be settable from a script.
        jar.clear();
        assert!(jar.set_from_script("evil=1; HttpOnly", &u));
        assert!(jar.script_cookie_string(&u).contains("evil=1"));
        assert!(jar.get_cookies(&u)[0].is_script_visible());
    }

    #[test]
    fn test_profile_persistence_roundtrip() {
        let mut jar = CookieJar::new();
        let u = url("https://news.ycombinator.com/item?id=1");
        jar.store_cookie(
            CookieJar::parse_set_cookie(
                "user=alice; Domain=news.ycombinator.com; Path=/; Max-Age=86400; SameSite=Lax; Secure",
                &u,
            )
            .unwrap(),
        );
        jar.store_cookie(CookieJar::parse_set_cookie("anon_session=1; Path=/item", &u).unwrap());

        let serialized = jar.to_profile_string();
        assert!(serialized.contains("news.ycombinator.com"));

        let mut restored = CookieJar::new();
        restored.load_profile_string(&serialized);
        assert_eq!(restored.len(), 2);

        let a = jar.all();
        let b = restored.all();
        assert_eq!(a.len(), b.len());
        for x in &a {
            let y = b
                .iter()
                .find(|c| c.name == x.name)
                .expect("cookie preserved");
            assert_eq!(x.value, y.value);
            assert_eq!(x.domain, y.domain);
            assert_eq!(x.path, y.path);
            assert_eq!(x.secure, y.secure);
            assert_eq!(x.http_only, y.http_only);
            assert_eq!(x.same_site, y.same_site);
            assert_eq!(x.host_only, y.host_only);
            assert_eq!(x.expires_at, y.expires_at);
        }
        // The Max-Age cookie survived as an absolute expiry timestamp.
        let with_expiry = b.iter().find(|c| c.name == "user").unwrap();
        assert!(with_expiry.expires_at.is_some());
    }

    #[test]
    fn test_profile_file_roundtrip_on_disk() {
        let dir = std::env::temp_dir().join(format!("mango_cookies_{}", std::process::id()));
        let path = dir.join("cookies.txt");
        let mut jar = CookieJar::new();
        let u = url("https://example.com/");
        jar.store_cookie(CookieJar::parse_set_cookie("k=v; Max-Age=600", &u).unwrap());
        jar.save_to_file(&path).expect("save");

        let mut restored = CookieJar::new();
        let count = restored.load_from_file(&path).expect("load");
        assert_eq!(count, 1);
        assert_eq!(restored.get_cookie_header(&u).unwrap(), "k=v");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_expired_cookies_are_pruned_and_not_sent() {
        let mut jar = CookieJar::new();
        let u = url("https://example.com/");
        let mut cookie = CookieJar::parse_set_cookie("old=1", &u).unwrap();
        cookie.expires_at = Some(1); // 1970-01-01
        jar.cookies.push(cookie);
        assert!(jar.get_cookie_header(&u).is_none());
        assert_eq!(jar.prune_expired(), 1);
        assert!(jar.is_empty());
    }

    #[test]
    fn test_expires_header_parsing() {
        // RFC 1123
        assert_eq!(
            parse_http_date("Wed, 21 Oct 2015 07:28:00 GMT"),
            Some(1_445_412_480)
        );
        // RFC 850
        assert_eq!(
            parse_http_date("Wednesday, 21-Oct-15 07:28:00 GMT"),
            Some(1_445_412_480)
        );
        // asctime
        assert_eq!(
            parse_http_date("Wed Oct 21 07:28:00 2015"),
            Some(1_445_412_480)
        );
        assert_eq!(parse_http_date("not a date"), None);
    }

    #[test]
    fn test_https_expires_attribute_marks_cookie_deleted() {
        let u = url("https://example.com/");
        let c = CookieJar::parse_set_cookie(
            "sess=1; Expires=Thu, 01 Jan 1970 00:00:00 GMT; Path=/",
            &u,
        )
        .unwrap();
        assert_eq!(c.value, "");
        assert!(c.is_expired());
    }

    #[test]
    fn test_longer_paths_come_first_in_cookie_header() {
        let mut jar = CookieJar::new();
        let u = url("https://example.com/deep/nested/page");
        jar.store_cookie(CookieJar::parse_set_cookie("a=root; Path=/", &u).unwrap());
        jar.store_cookie(CookieJar::parse_set_cookie("b=deep; Path=/deep/nested", &u).unwrap());
        let header = jar.get_cookie_header(&u).unwrap();
        assert!(header.starts_with("b=deep"), "got {header}");
    }

    #[test]
    fn test_samesite_attribute_parsing() {
        let u = url("https://example.com/");
        assert_eq!(
            CookieJar::parse_set_cookie("a=1; SameSite=Strict", &u)
                .unwrap()
                .same_site,
            SameSite::Strict
        );
        assert_eq!(
            CookieJar::parse_set_cookie("a=1; SameSite=lax", &u)
                .unwrap()
                .same_site,
            SameSite::Lax
        );
        assert_eq!(
            CookieJar::parse_set_cookie("b=1", &u).unwrap().same_site,
            SameSite::Unspecified
        );
    }

    #[test]
    fn test_profile_line_escapes_control_characters() {
        let mut jar = CookieJar::new();
        let u = url("https://example.com/");
        let mut cookie = CookieJar::parse_set_cookie("weird=v", &u).unwrap();
        cookie.value = "tab\there\nnext".to_string();
        jar.store_cookie(cookie);

        let body = jar.to_profile_string();
        assert_eq!(body.lines().count(), 3); // 2 comment lines + 1 cookie line

        let mut restored = CookieJar::new();
        restored.load_profile_string(&body);
        assert_eq!(restored.all()[0].value, "tab\there\nnext");
    }

    #[test]
    fn test_rfc6265_cross_domain_rejection() {
        let u = url("https://evil.com/");
        // Setting cookie for bank.com from evil.com must be rejected
        assert!(CookieJar::parse_set_cookie("auth=stolen; Domain=bank.com", &u).is_none());
        assert!(CookieJar::parse_set_cookie("auth=stolen; Domain=other.evil.com", &u).is_none());
    }

    #[test]
    fn test_rfc6265_public_suffix_rejection() {
        let u = url("https://example.com/");
        // Setting cookie for top-level domain .com must be rejected
        assert!(CookieJar::parse_set_cookie("tracker=1; Domain=com", &u).is_none());
        assert!(CookieJar::parse_set_cookie("tracker=1; Domain=.com", &u).is_none());

        let uk = url("https://shop.co.uk/");
        // Multi-part TLD .co.uk must be rejected
        assert!(CookieJar::parse_set_cookie("tracker=1; Domain=co.uk", &uk).is_none());
    }

    #[test]
    fn test_rfc6265_ip_address_matching() {
        let u = url("https://192.168.1.100/");
        // IP address can only match exact host
        assert!(CookieJar::parse_set_cookie("k=v; Domain=1.100", &u).is_none());
        let c = CookieJar::parse_set_cookie("k=v; Domain=192.168.1.100", &u).unwrap();
        assert_eq!(c.domain, "192.168.1.100");

        let mut jar = CookieJar::new();
        jar.store_cookie(c);
        assert!(jar.get_cookie_header(&u).is_some());

        let other_ip = url("https://192.168.1.200/");
        assert!(jar.get_cookie_header(&other_ip).is_none());
    }

    #[test]
    fn test_rfc6265bis_cookie_prefixes() {
        let secure_url = url("https://example.com/");
        let insecure_url = url("http://example.com/");

        // __Secure- requires HTTPS and Secure attribute
        assert!(CookieJar::parse_set_cookie("__Secure-sess=1", &secure_url).is_none()); // missing Secure
        assert!(CookieJar::parse_set_cookie("__Secure-sess=1; Secure", &insecure_url).is_none()); // not HTTPS
        assert!(CookieJar::parse_set_cookie("__Secure-sess=1; Secure", &secure_url).is_some());

        // __Host- requires HTTPS, Secure, Path=/, and NO Domain attribute
        assert!(
            CookieJar::parse_set_cookie("__Host-id=1; Secure; Path=/", &insecure_url).is_none()
        ); // not HTTPS
        assert!(CookieJar::parse_set_cookie("__Host-id=1; Path=/", &secure_url).is_none()); // missing Secure
        assert!(
            CookieJar::parse_set_cookie("__Host-id=1; Secure; Path=/sub", &secure_url).is_none()
        ); // path not /
        assert!(
            CookieJar::parse_set_cookie(
                "__Host-id=1; Secure; Domain=example.com; Path=/",
                &secure_url
            )
            .is_none()
        ); // Domain present
        assert!(CookieJar::parse_set_cookie("__Host-id=1; Secure; Path=/", &secure_url).is_some());
    }

    #[test]
    fn test_samesite_none_requires_secure() {
        let u = url("https://example.com/");
        // SameSite=None without Secure must be rejected
        assert!(CookieJar::parse_set_cookie("track=1; SameSite=None", &u).is_none());
        assert!(CookieJar::parse_set_cookie("track=1; SameSite=None; Secure", &u).is_some());
    }

    #[test]
    fn test_sorting_by_creation_time_on_equal_path_length() {
        let mut jar = CookieJar::new();
        let u = url("https://example.com/app");
        let mut c1 = CookieJar::parse_set_cookie("first=1; Path=/app", &u).unwrap();
        c1.creation_time = 100;
        let mut c2 = CookieJar::parse_set_cookie("second=2; Path=/app", &u).unwrap();
        c2.creation_time = 200;
        jar.store_cookie(c2);
        jar.store_cookie(c1);

        let header = jar.get_cookie_header(&u).unwrap();
        // first has earlier creation_time (100 < 200) so it must come first
        assert!(header.starts_with("first=1"), "got {header}");
    }

    #[test]
    fn test_quoted_attributes_and_positive_max_age() {
        let u = url("https://example.com/");
        let c = CookieJar::parse_set_cookie(
            r#"val="hello world"; Path="/"; Max-Age="+3600"; SameSite="Strict""#,
            &u,
        )
        .unwrap();
        assert_eq!(c.value, "hello world");
        assert_eq!(c.path, "/");
        assert_eq!(c.same_site, SameSite::Strict);
        assert!(c.expires_at.unwrap() > now_secs());
    }

    #[test]
    fn test_clear_session_cookies() {
        let mut jar = CookieJar::new();
        let u = url("https://example.com/");
        jar.store_cookie(CookieJar::parse_set_cookie("session=1", &u).unwrap());
        jar.store_cookie(CookieJar::parse_set_cookie("persist=2; Max-Age=3600", &u).unwrap());
        assert_eq!(jar.len(), 2);
        let dropped = jar.clear_session_cookies();
        assert_eq!(dropped, 1);
        assert_eq!(jar.len(), 1);
        assert_eq!(jar.all()[0].name, "persist");
    }
}

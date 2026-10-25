//! RFC 6265 compliant CookieJar storage, eviction, and profile persistence.

use std::path::{Path, PathBuf};

use crate::url::Url;

use super::cookie::{Cookie, now_secs};
use super::date::parse_http_date;
use super::public_suffix::{
    get_registrable_domain, is_ip_address, is_public_suffix, is_same_site_hosts,
};
use super::same_site::{CookiePriority, SameSite};

/// Maximum cookies per host (RFC 6265 §6.3 / Chromium standard: 180).
pub const MAX_COOKIES_PER_HOST: usize = 180;
/// Maximum bytes per cookie value (4KB).
pub const MAX_COOKIE_BYTES: usize = 4096;
/// Maximum cookies globally.
pub const MAX_COOKIES_TOTAL: usize = 3000;
/// Maximum profile file bytes read from disk (10MB DoS cap).
pub const MAX_PROFILE_READ_BYTES: usize = 10 * 1024 * 1024;

/// An RFC 6265 compliant cookie storage jar.
///
/// # Concurrency & Synchronization Invariant
/// When shared across threads (e.g., via `Arc<Mutex<CookieJar>>`), callers MUST
/// hold the mutex guard across the entire read-modify-write cycle. All mutating
/// operations (`store_cookie`, `set_from_script`, `remove_for_domain`, etc.)
/// require `&mut self` and perform atomic lookup-and-replacement without releasing
/// internal state, preventing TOCTOU races.
#[derive(Debug, Clone, Default)]
pub struct CookieJar {
    pub(crate) cookies: Vec<Cookie>,
}

impl CookieJar {
    /// Creates an empty cookie jar.
    pub fn new() -> Self {
        Self {
            cookies: Vec::new(),
        }
    }

    /// Parses a `Set-Cookie` response header value for a given request URL and optional top-level partition key (CHIPS).
    ///
    /// Validates attributes: Max-Age, Expires, Domain, Path, Secure, HttpOnly, SameSite, Partitioned, Priority,
    /// and enforces RFC 6265 / RFC 6265bis domain-matching, public suffix restrictions,
    /// SameSite=None requirements, and `__Secure-` / `__Host-` prefixes.
    pub fn parse_set_cookie_with_partition(
        header_value: &str,
        request_url: &Url,
        top_level_partition_key: Option<&str>,
    ) -> Option<Cookie> {
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

        let mut domain = None;
        let mut explicit_domain = false;
        let mut path = default_cookie_path(&request_url.path);
        let mut secure = false;
        let mut http_only = false;
        let mut is_deleted = false;
        let mut is_partitioned = false;
        let mut priority = CookiePriority::Medium;
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
                            domain = Some(clean_domain);
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
                            Ok(secs) => {
                                // RFC 6265bis §5.4: Cap Max-Age to at most 400 days and use saturating arithmetic.
                                const MAX_COOKIE_LIFETIME_SECS: u64 = 400 * 86_400;
                                let clamped_secs = (secs as u64).min(MAX_COOKIE_LIFETIME_SECS);
                                expires_at = Some(now_secs().saturating_add(clamped_secs));
                            }
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
                            // RFC 6265bis §5.4: Cap Expires to at most 400 days in the future
                            let max_future = now_secs().saturating_add(400 * 86_400);
                            expires_at = Some(ts.min(max_future));
                        }
                    }
                    "samesite" => same_site = SameSite::from_str(val),
                    "priority" => priority = CookiePriority::from_str(val),
                    "secure" => secure = true,
                    "httponly" => http_only = true,
                    "partitioned" => is_partitioned = true,
                    _ => {}
                }
            } else {
                let attr_lower = trimmed.to_ascii_lowercase();
                match attr_lower.as_str() {
                    "secure" => secure = true,
                    "httponly" => http_only = true,
                    "partitioned" => is_partitioned = true,
                    _ => {}
                }
            }
        }

        // RFC 6265bis §5.4: Secure cookies must only be accepted over secure transport (HTTPS)
        if secure && !is_https {
            return None;
        }

        // RFC 6265bis / CHIPS: Partitioned cookies MUST have the Secure attribute and use HTTPS
        if is_partitioned && (!secure || !is_https) {
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

        let partition_key = if is_partitioned {
            match top_level_partition_key {
                Some(pk) => Some(pk.trim().to_ascii_lowercase()),
                None => Some(format!(
                    "{}://{}",
                    if is_https { "https" } else { "http" },
                    get_registrable_domain(&request_host)
                )),
            }
        } else {
            None
        };

        let domain = domain.unwrap_or(request_host);
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
                partition_key,
                priority,
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
            partition_key,
            priority,
        })
    }

    /// Parses a `Set-Cookie` response header value for a given request URL.
    pub fn parse_set_cookie(header_value: &str, request_url: &Url) -> Option<Cookie> {
        Self::parse_set_cookie_with_partition(header_value, request_url, None)
    }

    /// Stores or deletes a cookie in the jar.
    ///
    /// An empty value alone does NOT delete — only expiry (`Max-Age<=0`,
    /// past `Expires`) removes. Cookie names are case-sensitive per RFC 6265.
    /// Enforces 4KB per-cookie (value truncated), 180 per-host, 3000 global.
    pub fn store_cookie(&mut self, mut cookie: Cookie) {
        if cookie.is_expired() {
            self.remove_matching(&cookie);
            return;
        }

        // Enforce 4KB per-cookie cap (truncate value on UTF-8 boundary).
        if cookie.value.len() > MAX_COOKIE_BYTES {
            let mut end = MAX_COOKIE_BYTES;
            while end > 0 && !cookie.value.is_char_boundary(end) {
                end -= 1;
            }
            cookie.value.truncate(end);
        }

        let now = now_secs();
        if let Some(existing) = self.cookies.iter_mut().find(|c| {
            c.name == cookie.name
                && c.domain == cookie.domain
                && c.path == cookie.path
                && c.partition_key == cookie.partition_key
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
            self.cookies.push(cookie.clone());
            self.enforce_per_host_limit(&cookie.domain);
        }

        self.enforce_limits();
    }

    /// Evicts oldest cookies for `domain` when exceeding MAX_COOKIES_PER_HOST (180 per-host)
    /// using Chromium priority-LRU eviction (Low -> Medium -> High).
    fn enforce_per_host_limit(&mut self, domain: &str) {
        let count = self.cookies.iter().filter(|c| c.domain == domain).count();
        if count <= MAX_COOKIES_PER_HOST {
            return;
        }
        let drop_count = count - MAX_COOKIES_PER_HOST;
        let mut idxs: Vec<usize> = self
            .cookies
            .iter()
            .enumerate()
            .filter(|(_, c)| c.domain == domain)
            .map(|(i, _)| i)
            .collect();
        idxs.sort_by(|&a, &b| {
            let ca = &self.cookies[a];
            let cb = &self.cookies[b];
            ca.priority
                .cmp(&cb.priority)
                .then_with(|| ca.last_access_time.cmp(&cb.last_access_time))
        });
        let mut to_remove: Vec<usize> = idxs.into_iter().take(drop_count).collect();
        to_remove.sort_unstable_by(|a, b| b.cmp(a));
        for i in to_remove {
            self.cookies.remove(i);
        }
    }

    fn enforce_limits(&mut self) {
        if self.cookies.len() <= MAX_COOKIES_TOTAL {
            return;
        }
        self.prune_expired();
        if self.cookies.len() > MAX_COOKIES_TOTAL {
            self.cookies.sort_by_key(|c| c.last_access_time);
            let drop_count = self.cookies.len() - MAX_COOKIES_TOTAL;
            self.cookies.drain(0..drop_count);
        }
    }

    fn remove_matching(&mut self, cookie: &Cookie) {
        self.cookies.retain(|c| {
            !(c.name == cookie.name
                && c.domain == cookie.domain
                && c.path == cookie.path
                && c.partition_key == cookie.partition_key)
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

    /// Returns all cookies matching the given URL, same-site context, and partition key, longest-path first.
    pub fn get_cookies_with_context_and_partition(
        &self,
        url: &Url,
        initiator_url: Option<&Url>,
        is_top_level_navigation: bool,
        is_safe_method: bool,
        partition_key: Option<&str>,
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

                // CHIPS Partitioning matching:
                if let Some(ref c_pk) = c.partition_key {
                    match partition_key {
                        Some(pk) if pk.eq_ignore_ascii_case(c_pk) => {}
                        _ => return false,
                    }
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

    /// Returns all cookies matching the given URL and same-site context, longest-path first.
    pub fn get_cookies_with_context(
        &self,
        url: &Url,
        initiator_url: Option<&Url>,
        is_top_level_navigation: bool,
        is_safe_method: bool,
    ) -> Vec<Cookie> {
        self.get_cookies_with_context_and_partition(
            url,
            initiator_url,
            is_top_level_navigation,
            is_safe_method,
            None,
        )
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

    /// Returns `true` if the jar holds zero cookies.
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
    /// Enforces a 10MB read cap to prevent DoS via huge profile files.
    pub fn load_from_file(&mut self, path: &Path) -> std::io::Result<usize> {
        let meta = match std::fs::metadata(path) {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(0),
            Err(e) => return Err(e),
        };
        if meta.len() > MAX_PROFILE_READ_BYTES as u64 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Cookie profile exceeds {} byte cap", MAX_PROFILE_READ_BYTES),
            ));
        }
        match std::fs::read_to_string(path) {
            Ok(body) => {
                if body.len() > MAX_PROFILE_READ_BYTES {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "Cookie profile exceeds read cap",
                    ));
                }
                self.load_profile_string(&body);
                Ok(self.cookies.len())
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(0),
            Err(e) => Err(e),
        }
    }

    /// Writes the jar to a profile file atomically, creating parent directories as needed.
    /// Uses a `.tmp` suffix + `fsync` + atomic rename, with `0600` permissions on Unix.
    pub fn save_to_file(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)?;
        }
        let tmp_path = {
            let mut s = path.as_os_str().to_owned();
            s.push(".tmp");
            PathBuf::from(s)
        };
        let body = self.to_profile_string();
        // Write temp file.
        {
            use std::io::Write as _;
            let mut options = std::fs::OpenOptions::new();
            options.write(true).create(true).truncate(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt as _;
                options.mode(0o600);
            }
            let mut f = options.open(&tmp_path)?;
            f.write_all(body.as_bytes())?;
            f.sync_all()?;
        }
        // Ensure 0600 even if file already existed (create may not reset mode).
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let _ = std::fs::set_permissions(&tmp_path, std::fs::Permissions::from_mode(0o600));
        }
        std::fs::rename(&tmp_path, path)?;
        // Fsync parent directory for durability.
        #[cfg(unix)]
        {
            if let Some(parent) = path.parent() {
                if let Ok(dir) = std::fs::File::open(parent) {
                    let _ = dir.sync_all();
                }
            }
        }
        Ok(())
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
pub fn default_cookie_path(request_path: &str) -> String {
    if request_path.is_empty() || !request_path.starts_with('/') {
        return "/".to_string();
    }
    match request_path.rfind('/') {
        Some(0) => "/".to_string(),
        Some(idx) => request_path[..idx].to_string(),
        None => "/".to_string(),
    }
}

pub fn cookie_domain_matches(url_host: &str, cookie_domain: &str, host_only: bool) -> bool {
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

pub fn path_matches(url_path: &str, cookie_path: &str) -> bool {
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

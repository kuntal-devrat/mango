use std::collections::HashMap;
use std::time::Duration;

use super::types::{
    HttpResponse, NetworkError, create_pooled_agent, insert_response_header, map_ureq_error,
    read_body_capped, read_body_streaming,
};
use crate::cookies::{Cookie, CookieJar};
use crate::url::Url;

/// A high-performance, pure-Rust HTTP client with automatic cookie management and connection pooling.
#[derive(Clone)]
pub struct HttpClient {
    pub user_agent: String,
    pub timeout: Duration,
    pub cookie_jar: std::sync::Arc<std::sync::Mutex<CookieJar>>,
    pub referrer_policies:
        std::sync::Arc<std::sync::Mutex<std::collections::HashMap<String, String>>>,
    pub agent: std::sync::Arc<ureq::Agent>,
}

impl std::fmt::Debug for HttpClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HttpClient")
            .field("user_agent", &self.user_agent)
            .field("timeout", &self.timeout)
            .finish()
    }
}

impl Default for HttpClient {
    fn default() -> Self {
        Self::new()
    }
}

impl HttpClient {
    /// Creates a new HTTP client with default settings and empty cookie jar.
    pub fn new() -> Self {
        Self::with_timeout(Duration::from_secs(30))
    }

    /// Creates an HTTP client with a custom timeout.
    pub fn with_timeout(timeout: Duration) -> Self {
        Self {
            user_agent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36".to_string(),
            timeout,
            cookie_jar: std::sync::Arc::new(std::sync::Mutex::new(CookieJar::new())),
            referrer_policies: std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
            agent: create_pooled_agent(timeout),
        }
    }

    /// Creates an HTTP client sharing an existing cookie jar.
    pub fn with_cookie_jar(cookie_jar: std::sync::Arc<std::sync::Mutex<CookieJar>>) -> Self {
        Self::with_timeout_and_cookie_jar(Duration::from_secs(30), cookie_jar)
    }

    /// Creates an HTTP client sharing an existing cookie jar with custom timeout.
    pub fn with_timeout_and_cookie_jar(
        timeout: Duration,
        cookie_jar: std::sync::Arc<std::sync::Mutex<CookieJar>>,
    ) -> Self {
        Self {
            user_agent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36".to_string(),
            timeout,
            cookie_jar,
            referrer_policies: std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
            agent: create_pooled_agent(timeout),
        }
    }

    /// Sets a known Referrer-Policy for an origin.
    pub fn set_referrer_policy(&self, origin: &str, policy: &str) {
        if let Ok(mut map) = self.referrer_policies.lock() {
            map.insert(origin.to_string(), policy.to_string());
        }
    }

    /// Looks up the Referrer-Policy for a given page URL or origin.
    pub fn referrer_policy_for(&self, page_url: &Url) -> Option<String> {
        let origin = crate::security::Origin::parse(&page_url.as_str())?.to_string();
        self.referrer_policies.lock().ok()?.get(&origin).cloned()
    }

    /// Computes the Referer header value to attach to a request to `target_url` initiated by `initiator_url`.
    pub fn compute_referrer_header(
        &self,
        target_url: &Url,
        initiator_url: Option<&Url>,
    ) -> Option<String> {
        let initiator = initiator_url?;
        let policy_str = self.referrer_policy_for(initiator);
        crate::security::compute_referrer_with_policy_str(
            initiator,
            target_url,
            policy_str.as_deref(),
        )
    }

    /// Records the Referrer-Policy response header if present.
    fn record_referrer_policy(&self, url: &Url, headers: &HashMap<String, String>) {
        if let Some(rp) = headers.get("referrer-policy") {
            if let Some(orig) = crate::security::Origin::parse(&url.as_str()) {
                if let Ok(mut map) = self.referrer_policies.lock() {
                    map.insert(orig.to_string(), rp.clone());
                }
            }
        }
    }

    /// Clears all cookies in this client's jar.
    pub fn clear_cookies(&self) {
        if let Ok(mut jar) = self.cookie_jar.lock() {
            jar.clear();
        }
    }

    /// Returns matching cookies for the given URL.
    pub fn cookies_for_url(&self, url: &Url) -> Vec<Cookie> {
        self.cookie_jar
            .lock()
            .map(|jar| jar.get_cookies(url))
            .unwrap_or_default()
    }

    /// Executes a single HTTP GET request without following redirects.
    /// Top-level navigation context (`is_top_level=true`).
    pub fn get_single(&self, url: &Url) -> Result<HttpResponse, NetworkError> {
        self.get_single_with_context(url, None, true, true)
    }

    /// Executes a single HTTP GET with explicit SameSite initiator context.
    /// Subresources must pass `initiator` (page URL) with `is_top_level=false`.
    pub fn get_single_with_context(
        &self,
        url: &Url,
        initiator_url: Option<&Url>,
        is_top_level: bool,
        is_safe_method: bool,
    ) -> Result<HttpResponse, NetworkError> {
        let url_str = url.as_str();

        let mut req = self.agent
            .get(&url_str)
            .header("User-Agent", &self.user_agent)
            .header(
                "Accept",
                "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8,application/signed-exchange;v=b3;q=0.7",
            )
            .header("Accept-Language", "en-US,en;q=0.9")
            .header("Accept-Encoding", "gzip, deflate, br")
            .header("sec-ch-ua", "\"Chromium\";v=\"128\", \"Not;A=Brand\";v=\"24\", \"Google Chrome\";v=\"128\"")
            .header("sec-ch-ua-mobile", "?0")
            .header("sec-ch-ua-platform", "\"Windows\"")
            .header("upgrade-insecure-requests", "1");

        if let Ok(jar) = self.cookie_jar.lock()
            && let Some(cookie_hdr) =
                jar.get_cookie_header_with_context(url, initiator_url, is_top_level, is_safe_method)
        {
            req = req.header("Cookie", &cookie_hdr);
        }

        if let Some(ref_hdr) = self.compute_referrer_header(url, initiator_url) {
            req = req.header("Referer", &ref_hdr);
        }

        let response = req.call().map_err(|e| map_ureq_error(e, url))?;

        // Extract Set-Cookie headers into the cookie jar
        if let Ok(mut jar) = self.cookie_jar.lock() {
            for (name, val) in response.headers() {
                if name.as_str().eq_ignore_ascii_case("set-cookie")
                    && let Ok(v_str) = val.to_str()
                    && let Some(cookie) = CookieJar::parse_set_cookie(v_str, url)
                {
                    jar.store_cookie(cookie);
                }
            }
        }

        let status = response.status().as_u16();
        let status_text = response
            .status()
            .canonical_reason()
            .unwrap_or("")
            .to_string();

        let mut headers = HashMap::new();
        for (name, val) in response.headers() {
            if let Ok(v_str) = val.to_str() {
                insert_response_header(&mut headers, name.as_str(), v_str);
            }
        }

        self.record_referrer_policy(url, &headers);

        let body = read_body_capped(response.into_body())?;

        Ok(HttpResponse {
            final_url: url.clone(),
            status,
            status_text,
            headers,
            version: crate::http2::HttpVersion::Http11,
            body,
        })
    }

    /// Fetches a URL, following HTTP redirects up to `max_redirects` times.
    pub fn fetch(&self, start_url: &Url) -> Result<HttpResponse, NetworkError> {
        self.fetch_with_context(start_url, None, true, true)
    }

    /// Fetches a URL with explicit SameSite initiator context.
    /// Top-level navigations use `is_top_level=true`; subresources use
    /// `initiator=Some(page_url)` with `is_top_level=false`.
    pub fn fetch_with_context(
        &self,
        start_url: &Url,
        initiator_url: Option<&Url>,
        is_top_level: bool,
        is_safe_method: bool,
    ) -> Result<HttpResponse, NetworkError> {
        let mut current_url = start_url.clone();
        let mut redirects_followed = 0;
        let max_redirects = 5;
        let mut visited_urls = Vec::new();

        loop {
            if redirects_followed >= max_redirects {
                return Err(NetworkError::TooManyRedirects);
            }

            visited_urls.push(current_url.as_str());

            let resp = self.get_single_with_context(
                &current_url,
                initiator_url,
                is_top_level,
                is_safe_method,
            )?;

            if resp.is_redirect()
                && let Some(loc) = resp.location().map(|s| s.to_string())
            {
                let next_url = current_url
                    .resolve(&loc)
                    .map_err(|e| NetworkError::InvalidUrl(e.to_string()))?;

                if visited_urls.contains(&next_url.as_str()) {
                    return Err(NetworkError::TooManyRedirects); // Loop detected
                }

                log::info!("Following redirect {} -> {}", current_url, next_url);
                current_url = next_url;
                redirects_followed += 1;
                continue;
            }

            let mut final_resp = resp;
            final_resp.final_url = current_url;
            return Ok(final_resp);
        }
    }

    /// Fetches a URL with streaming response body callback.
    /// Follows redirects, handles cookies and HSTS, and streams final response chunks to `on_chunk`.
    pub fn fetch_streaming<F: FnMut(&[u8]) -> bool>(
        &self,
        start_url: &Url,
        on_chunk: F,
    ) -> Result<HttpResponse, NetworkError> {
        self.fetch_streaming_with_context(start_url, None, true, true, on_chunk)
    }

    /// Fetches a URL with explicit SameSite initiator context and streaming chunks.
    pub fn fetch_streaming_with_context<F: FnMut(&[u8]) -> bool>(
        &self,
        start_url: &Url,
        initiator_url: Option<&Url>,
        is_top_level: bool,
        is_safe_method: bool,
        mut on_chunk: F,
    ) -> Result<HttpResponse, NetworkError> {
        let mut current_url = start_url.clone();
        let mut redirects_followed = 0;
        let max_redirects = 5;
        let mut visited_urls = Vec::new();

        loop {
            if redirects_followed >= max_redirects {
                return Err(NetworkError::TooManyRedirects);
            }

            visited_urls.push(current_url.as_str());

            let url_str = current_url.as_str();
            let mut req = self
                .agent
                .get(&url_str)
                .header("User-Agent", &self.user_agent)
                .header(
                    "Accept",
                    "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8,application/signed-exchange;v=b3;q=0.7",
                )
                .header("Accept-Language", "en-US,en;q=0.9")
                .header("Accept-Encoding", "gzip, deflate, br")
                .header(
                    "sec-ch-ua",
                    "\"Chromium\";v=\"128\", \"Not;A=Brand\";v=\"24\", \"Google Chrome\";v=\"128\"",
                )
                .header("sec-ch-ua-mobile", "?0")
                .header("sec-ch-ua-platform", "\"Windows\"")
                .header("upgrade-insecure-requests", "1");

            if let Ok(jar) = self.cookie_jar.lock()
                && let Some(cookie_hdr) = jar.get_cookie_header_with_context(
                    &current_url,
                    initiator_url,
                    is_top_level,
                    is_safe_method,
                )
            {
                req = req.header("Cookie", &cookie_hdr);
            }

            if let Some(ref_hdr) = self.compute_referrer_header(&current_url, initiator_url) {
                req = req.header("Referer", &ref_hdr);
            }

            let response = req.call().map_err(|e| map_ureq_error(e, &current_url))?;

            if let Ok(mut jar) = self.cookie_jar.lock() {
                for (name, val) in response.headers() {
                    if name.as_str().eq_ignore_ascii_case("set-cookie")
                        && let Ok(v_str) = val.to_str()
                        && let Some(cookie) = CookieJar::parse_set_cookie(v_str, &current_url)
                    {
                        jar.store_cookie(cookie);
                    }
                }
            }

            let status = response.status().as_u16();
            let status_text = response
                .status()
                .canonical_reason()
                .unwrap_or("")
                .to_string();

            let mut headers = HashMap::new();
            for (name, val) in response.headers() {
                if let Ok(v_str) = val.to_str() {
                    insert_response_header(&mut headers, name.as_str(), v_str);
                }
            }

            self.record_referrer_policy(&current_url, &headers);

            let is_redirect = matches!(status, 301 | 302 | 303 | 307 | 308);
            if is_redirect && let Some(loc) = headers.get("location") {
                let _ = read_body_capped(response.into_body());
                let next_url = current_url
                    .resolve(loc)
                    .map_err(|e| NetworkError::InvalidUrl(e.to_string()))?;

                if visited_urls.contains(&next_url.as_str()) {
                    return Err(NetworkError::TooManyRedirects);
                }

                log::info!("Following redirect {} -> {}", current_url, next_url);
                current_url = next_url;
                redirects_followed += 1;
                continue;
            }

            let body = read_body_streaming(response.into_body(), &mut on_chunk)?;

            return Ok(HttpResponse {
                final_url: current_url,
                status,
                status_text,
                headers,
                version: crate::http2::HttpVersion::Http11,
                body,
            });
        }
    }

    /// Async non-blocking fetch with bounded timeout.
    ///
    /// Runs the blocking `ureq` fetch on Tokio's blocking pool so async
    /// executors are never stalled, and caps the wait with `timeout`
    /// (default 20s) to prevent indefinite hangs.
    pub async fn fetch_async(&self, url: &Url) -> Result<HttpResponse, NetworkError> {
        self.fetch_async_timeout(url, Duration::from_secs(20)).await
    }

    /// Async fetch with an explicit timeout.
    pub async fn fetch_async_timeout(
        &self,
        url: &Url,
        timeout: Duration,
    ) -> Result<HttpResponse, NetworkError> {
        let client = self.clone();
        let url = url.clone();
        let fut = tokio::task::spawn_blocking(move || client.fetch(&url));
        match tokio::time::timeout(timeout, fut).await {
            Err(_) => Err(NetworkError::Timeout),
            Ok(Err(e)) => Err(NetworkError::Other(format!(
                "HTTP fetch task panicked: {e}"
            ))),
            Ok(Ok(res)) => res,
        }
    }

    /// Async non-blocking single GET with bounded timeout.
    pub async fn get_single_async(&self, url: &Url) -> Result<HttpResponse, NetworkError> {
        let client = self.clone();
        let url = url.clone();
        let fut = tokio::task::spawn_blocking(move || client.get_single(&url));
        match tokio::time::timeout(Duration::from_secs(20), fut).await {
            Err(_) => Err(NetworkError::Timeout),
            Ok(Err(e)) => Err(NetworkError::Other(format!("HTTP GET task panicked: {e}"))),
            Ok(Ok(res)) => res,
        }
    }

    /// Sends an HTTP POST request with the given body bytes and Content-Type.
    ///
    /// Handles cookies and follows 301/302/303 Post/Redirect/Get redirects.
    pub fn post_with_body(
        &self,
        url: &Url,
        body: &[u8],
        content_type: &str,
    ) -> Result<HttpResponse, NetworkError> {
        let url_str = url.as_str();

        let mut req = self.agent.post(&url_str)
            .header("User-Agent", &self.user_agent)
            .header("Content-Type", content_type)
            .header(
                "Accept",
                "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8,application/signed-exchange;v=b3;q=0.7",
            )
            .header("Accept-Language", "en-US,en;q=0.9")
            .header("Accept-Encoding", "gzip, deflate, br")
            .header("sec-ch-ua", "\"Chromium\";v=\"128\", \"Not;A=Brand\";v=\"24\", \"Google Chrome\";v=\"128\"")
            .header("sec-ch-ua-mobile", "?0")
            .header("sec-ch-ua-platform", "\"Windows\"")
            .header("upgrade-insecure-requests", "1")
            .header("Sec-Fetch-Dest", "document")
            .header("Sec-Fetch-Mode", "navigate")
            .header("Sec-Fetch-Site", "same-origin")
            .header("Sec-Fetch-User", "?1");

        if let Some(host) = &url.host {
            let port_suffix = match (url.scheme.as_str(), url.port) {
                ("http", Some(80)) | ("https", Some(443)) | (_, None) => String::new(),
                (_, Some(p)) => format!(":{p}"),
            };
            let origin = format!("{}://{}{}", url.scheme, host, port_suffix);
            req = req.header("Origin", &origin);
        }
        // For form posts the initiator is the page itself; callers needing
        // subresource context should use request_with_context.
        if let Ok(jar) = self.cookie_jar.lock()
            && let Some(cookie_hdr) = jar.get_cookie_header_with_context(url, None, true, false)
        {
            req = req.header("Cookie", &cookie_hdr);
        }

        let resp = req.send(body).map_err(|e| map_ureq_error(e, url))?;

        // Extract Set-Cookie headers into the cookie jar
        if let Ok(mut jar) = self.cookie_jar.lock() {
            for (name, val) in resp.headers() {
                if name.as_str().eq_ignore_ascii_case("set-cookie")
                    && let Ok(v_str) = val.to_str()
                    && let Some(cookie) = CookieJar::parse_set_cookie(v_str, url)
                {
                    jar.store_cookie(cookie);
                }
            }
        }

        let status = resp.status().as_u16();
        let status_text = resp.status().canonical_reason().unwrap_or("").to_string();

        let mut headers = HashMap::new();
        for (name, val) in resp.headers() {
            if let Ok(v_str) = val.to_str() {
                insert_response_header(&mut headers, name.as_str(), v_str);
            }
        }

        // Handle Post/Redirect/Get (301, 302, 303)
        if (status == 301 || status == 302 || status == 303)
            && let Some(loc) = headers.get("location")
            && let Ok(redirect_url) = url.resolve(loc)
        {
            return self.fetch(&redirect_url);
        }

        let body_bytes = read_body_capped(resp.into_body())?;

        Ok(HttpResponse {
            final_url: url.clone(),
            status,
            status_text,
            headers,
            version: crate::http2::HttpVersion::Http11,
            body: body_bytes,
        })
    }

    /// Executes a general HTTP request with arbitrary method, custom headers, and optional body.
    ///
    /// Transmits cookies from the cookie jar, handles custom headers, records Set-Cookie responses,
    /// and returns status and body for all responses (including 4xx and 5xx).
    /// Defaults to top-level navigation context; use [`request_with_context`]
    /// for subresources (`is_top_level=false` + initiator).
    pub fn request(
        &self,
        method: &str,
        url: &Url,
        custom_headers: &[(String, String)],
        body: Option<&[u8]>,
    ) -> Result<HttpResponse, NetworkError> {
        // Heuristic: GET/HEAD with no initiator are treated as subresource-safe
        // only when explicitly routed via request_with_context. Default here
        // preserves top-level behavior for backwards compat.
        self.request_with_context(method, url, custom_headers, body, None, true, true)
    }

    /// Executes a general HTTP request with explicit SameSite initiator context.
    ///
    /// Subresources (fetch/XHR/CSS/JS/img) must pass the page URL as
    /// `initiator_url` with `is_top_level=false`; top-level navigations pass
    /// `is_top_level=true`. `is_safe_method` is true for GET/HEAD/OPTIONS/TRACE.
    pub fn request_with_context(
        &self,
        method: &str,
        url: &Url,
        custom_headers: &[(String, String)],
        body: Option<&[u8]>,
        initiator_url: Option<&Url>,
        is_top_level: bool,
        is_safe_method: bool,
    ) -> Result<HttpResponse, NetworkError> {
        let url_str = url.as_str();
        let upper_method = method.trim().to_uppercase();

        let has_custom_referer = custom_headers
            .iter()
            .any(|(k, _)| k.eq_ignore_ascii_case("referer"));
        let auto_referer = if !has_custom_referer {
            self.compute_referrer_header(url, initiator_url)
        } else {
            None
        };

        let resp = if upper_method == "GET" && body.is_none() {
            let mut req = self
                .agent
                .get(&url_str)
                .header("User-Agent", &self.user_agent)
                .header("Accept", "*/*")
                .header("Accept-Language", "en-US,en;q=0.9")
                .header("Accept-Encoding", "gzip, deflate, br");
            if let Ok(jar) = self.cookie_jar.lock()
                && let Some(cookie_hdr) = jar.get_cookie_header_with_context(
                    url,
                    initiator_url,
                    is_top_level,
                    is_safe_method,
                )
            {
                req = req.header("Cookie", &cookie_hdr);
            }
            if let Some(ref r) = auto_referer {
                req = req.header("Referer", r);
            }
            for (k, v) in custom_headers {
                req = req.header(k.as_str(), v.as_str());
            }
            req.call()
        } else if upper_method == "HEAD" {
            let mut req = self
                .agent
                .head(&url_str)
                .header("User-Agent", &self.user_agent)
                .header("Accept", "*/*")
                .header("Accept-Language", "en-US,en;q=0.9")
                .header("Accept-Encoding", "gzip, deflate, br");
            if let Ok(jar) = self.cookie_jar.lock()
                && let Some(cookie_hdr) = jar.get_cookie_header_with_context(
                    url,
                    initiator_url,
                    is_top_level,
                    is_safe_method,
                )
            {
                req = req.header("Cookie", &cookie_hdr);
            }
            if let Some(ref r) = auto_referer {
                req = req.header("Referer", r);
            }
            for (k, v) in custom_headers {
                req = req.header(k.as_str(), v.as_str());
            }
            req.call()
        } else if upper_method == "DELETE" {
            let mut req = self
                .agent
                .delete(&url_str)
                .header("User-Agent", &self.user_agent)
                .header("Accept", "*/*")
                .header("Accept-Language", "en-US,en;q=0.9")
                .header("Accept-Encoding", "gzip, deflate, br");
            if let Ok(jar) = self.cookie_jar.lock()
                && let Some(cookie_hdr) = jar.get_cookie_header_with_context(
                    url,
                    initiator_url,
                    is_top_level,
                    is_safe_method,
                )
            {
                req = req.header("Cookie", &cookie_hdr);
            }
            if let Some(ref r) = auto_referer {
                req = req.header("Referer", r);
            }
            for (k, v) in custom_headers {
                req = req.header(k.as_str(), v.as_str());
            }
            req.call()
        } else if upper_method == "OPTIONS" {
            let mut req = self
                .agent
                .options(&url_str)
                .header("User-Agent", &self.user_agent)
                .header("Accept", "*/*")
                .header("Accept-Language", "en-US,en;q=0.9")
                .header("Accept-Encoding", "gzip, deflate, br");
            if let Ok(jar) = self.cookie_jar.lock()
                && let Some(cookie_hdr) = jar.get_cookie_header_with_context(
                    url,
                    initiator_url,
                    is_top_level,
                    is_safe_method,
                )
            {
                req = req.header("Cookie", &cookie_hdr);
            }
            if let Some(ref r) = auto_referer {
                req = req.header("Referer", r);
            }
            for (k, v) in custom_headers {
                req = req.header(k.as_str(), v.as_str());
            }
            req.call()
        } else {
            let mut req = match upper_method.as_str() {
                "PUT" => self.agent.put(&url_str),
                "PATCH" => self.agent.patch(&url_str),
                _ => self.agent.post(&url_str),
            };
            req = req
                .header("User-Agent", &self.user_agent)
                .header("Accept", "*/*")
                .header("Accept-Language", "en-US,en;q=0.9")
                .header("Accept-Encoding", "gzip, deflate, br");
            if let Ok(jar) = self.cookie_jar.lock()
                && let Some(cookie_hdr) = jar.get_cookie_header_with_context(
                    url,
                    initiator_url,
                    is_top_level,
                    is_safe_method,
                )
            {
                req = req.header("Cookie", &cookie_hdr);
            }
            if let Some(ref r) = auto_referer {
                req = req.header("Referer", r);
            }
            for (k, v) in custom_headers {
                req = req.header(k.as_str(), v.as_str());
            }
            match body {
                Some(b) => req.send(b),
                None => req.send_empty(),
            }
        }
        .map_err(|e| map_ureq_error(e, url))?;

        // Extract Set-Cookie headers into the cookie jar
        if let Ok(mut jar) = self.cookie_jar.lock() {
            for (name, val) in resp.headers() {
                if name.as_str().eq_ignore_ascii_case("set-cookie")
                    && let Ok(v_str) = val.to_str()
                    && let Some(cookie) = CookieJar::parse_set_cookie(v_str, url)
                {
                    jar.store_cookie(cookie);
                }
            }
        }

        let status = resp.status().as_u16();
        let status_text = resp.status().canonical_reason().unwrap_or("").to_string();

        let mut headers = HashMap::new();
        for (name, val) in resp.headers() {
            if let Ok(v_str) = val.to_str() {
                insert_response_header(&mut headers, name.as_str(), v_str);
            }
        }

        self.record_referrer_policy(url, &headers);

        let body_bytes = read_body_capped(resp.into_body())?;

        Ok(HttpResponse {
            final_url: url.clone(),
            status,
            status_text,
            headers,
            version: crate::http2::HttpVersion::Http11,
            body: body_bytes,
        })
    }
}

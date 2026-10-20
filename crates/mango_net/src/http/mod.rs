//! HTTP client, request/response models, and redirect handling.

pub mod client;
pub mod types;

#[cfg(test)]
mod tests;

pub use crate::cookies::{Cookie, CookieJar, SameSite};
pub use client::HttpClient;
pub use types::{
    HttpMethod, HttpRequest, HttpResponse, MAX_RESPONSE_BODY_BYTES, NetworkError,
    insert_response_header, read_body_streaming,
};

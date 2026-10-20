//! HTTP security policy handling: CSP, HSTS, CORS, Subresource Integrity,
//! Referrer-Policy, Certificate Pinning, and Mixed Content blocking.

pub mod cors;
pub mod crypto;
pub mod csp;
pub mod hsts;
pub mod pinning;
pub mod referrer;
pub mod sri;

#[cfg(test)]
mod tests;

pub use cors::{
    CorsError, Origin, RequestCredentials, is_cors_simple_request, is_same_origin,
    validate_cors_preflight, validate_cors_response, validate_cors_response_with_credentials,
};
pub use crypto::{sha256, sha384, sha512};
pub use csp::{CspPolicy, CspViolationReport, ResourceDirective};
pub use hsts::{
    HstsDirective, HstsStore, MAX_HSTS_ENTRIES, SecurityHeaders, blocks_mixed_content,
    upgrade_to_https,
};
pub use pinning::{CertificatePinSet, PinError, ProxyConfig};
pub use referrer::{
    ReferrerPolicy, compute_referrer, compute_referrer_from_str, compute_referrer_with_policy_str,
};
pub use sri::{SriAlgorithm, SriError, SriItem, parse_sri_metadata, verify_subresource_integrity};

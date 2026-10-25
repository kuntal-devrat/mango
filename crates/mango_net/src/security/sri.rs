//! Subresource Integrity (SRI) engine conforming to W3C Subresource Integrity §3.3.

use super::crypto::{sha256, sha384, sha512};

/// Supported cryptographic hash algorithms for Subresource Integrity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SriAlgorithm {
    Sha256 = 1,
    Sha384 = 2,
    Sha512 = 3,
}

impl SriAlgorithm {
    pub fn from_prefix(prefix: &str) -> Option<Self> {
        match prefix.to_ascii_lowercase().as_str() {
            "sha256" => Some(SriAlgorithm::Sha256),
            "sha384" => Some(SriAlgorithm::Sha384),
            "sha512" => Some(SriAlgorithm::Sha512),
            _ => None,
        }
    }
}

/// An individual SRI hash expression consisting of algorithm and expected base64 digest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SriItem {
    pub algorithm: SriAlgorithm,
    pub digest_base64: String,
}

/// Errors occurring during Subresource Integrity validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SriError {
    NoValidMetadata,
    DigestMismatch {
        algorithm: SriAlgorithm,
        expected: String,
        actual: String,
    },
}

impl std::fmt::Display for SriError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SriError::NoValidMetadata => {
                write!(f, "No valid cryptographic hash found in SRI metadata")
            }
            SriError::DigestMismatch {
                algorithm,
                expected,
                actual,
            } => {
                write!(
                    f,
                    "SRI {algorithm:?} digest mismatch: expected '{expected}', calculated '{actual}'"
                )
            }
        }
    }
}

impl std::error::Error for SriError {}

/// Parses the integrity attribute value into a list of recognized SRI items.
pub fn parse_sri_metadata(integrity_attr: &str) -> Vec<SriItem> {
    let mut items = Vec::new();
    for token in integrity_attr.split_whitespace() {
        if let Some((algo_prefix, digest)) = token.split_once('-') {
            if let Some(algorithm) = SriAlgorithm::from_prefix(algo_prefix) {
                // Remove optional options suffix '?options'
                let clean_digest = digest.split('?').next().unwrap_or(digest).trim();
                if !clean_digest.is_empty() {
                    items.push(SriItem {
                        algorithm,
                        digest_base64: clean_digest.to_string(),
                    });
                }
            }
        }
    }
    items
}

/// Verifies resource content against the given SRI `integrity` attribute.
/// Conforms to W3C Subresource Integrity §3.3.
pub fn verify_subresource_integrity(bytes: &[u8], integrity_attr: &str) -> Result<bool, SriError> {
    let items = parse_sri_metadata(integrity_attr);
    if items.is_empty() {
        return Ok(true); // Empty or unrecognized integrity expression is ignored by spec
    }

    // Select the strongest algorithm present
    let Some(strongest_algo) = items.iter().map(|item| item.algorithm).max() else {
        return Ok(true);
    };
    let candidates: Vec<&SriItem> = items
        .iter()
        .filter(|i| i.algorithm == strongest_algo)
        .collect();

    let actual_base64 = match strongest_algo {
        SriAlgorithm::Sha256 => crate::websocket::base64_encode(&sha256(bytes)),
        SriAlgorithm::Sha384 => crate::websocket::base64_encode(&sha384(bytes)),
        SriAlgorithm::Sha512 => crate::websocket::base64_encode(&sha512(bytes)),
    };

    // If any candidate hash matches, the resource is authentic
    for cand in &candidates {
        if cand.digest_base64 == actual_base64 {
            return Ok(true);
        }
    }

    let expected_digest = candidates
        .first()
        .map(|c| c.digest_base64.clone())
        .unwrap_or_default();

    Err(SriError::DigestMismatch {
        algorithm: strongest_algo,
        expected: expected_digest,
        actual: actual_base64,
    })
}

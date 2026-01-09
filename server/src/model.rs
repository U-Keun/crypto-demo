use serde::{Deserialize, Serialize};

/// Request for /encrypt endpoint
#[derive(Debug, Deserialize)]
pub struct EncryptRequest {
    /// Additional Authenticated Data (optional)
    #[serde(default)]
    pub aad: Option<String>,
    /// Base64-encoded plaintext
    pub plaintext_b64: String,
}

/// Response from /encrypt endpoint
#[derive(Debug, Serialize)]
pub struct EncryptResponse {
    /// Key version (always 1 for MVP)
    pub keyver: u32,
    /// Base64-encoded nonce (12 bytes)
    pub nonce_b64: String,
    /// Base64-encoded ciphertext
    pub ciphertext_b64: String,
    /// Base64-encoded authentication tag (16 bytes)
    pub tag_b64: String,
}

/// Request for /decrypt endpoint
#[derive(Debug, Deserialize)]
pub struct DecryptRequest {
    /// Additional Authenticated Data (must match encryption AAD)
    #[serde(default)]
    pub aad: Option<String>,
    /// Key version
    pub keyver: u32,
    /// Base64-encoded nonce (12 bytes)
    pub nonce_b64: String,
    /// Base64-encoded ciphertext
    pub ciphertext_b64: String,
    /// Base64-encoded authentication tag (16 bytes)
    pub tag_b64: String,
}

/// Response from /decrypt endpoint
#[derive(Debug, Serialize)]
pub struct DecryptResponse {
    /// Base64-encoded plaintext
    pub plaintext_b64: String,
}

/// Error response
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
}

impl ErrorResponse {
    pub fn new(error: impl Into<String>) -> Self {
        Self {
            error: error.into(),
            details: None,
        }
    }

    pub fn with_details(error: impl Into<String>, details: impl Into<String>) -> Self {
        Self {
            error: error.into(),
            details: Some(details.into()),
        }
    }
}

use crate::crypto_ffi::{CryptoProvider, CRYPTO_KEY_SIZE, CRYPTO_NONCE_SIZE, CRYPTO_TAG_SIZE};
use crate::model::{DecryptRequest, DecryptResponse, EncryptRequest, EncryptResponse, ErrorResponse};
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use std::sync::{Arc, Mutex};

/// Application state shared across handlers
#[derive(Clone)]
pub struct AppState {
    pub provider: Arc<Mutex<CryptoProvider>>,
}

impl AppState {
    /// Create new application state with a crypto provider
    pub fn new(key: &[u8; CRYPTO_KEY_SIZE]) -> Result<Self, String> {
        let provider = CryptoProvider::new(key).map_err(|e| e.to_string())?;
        Ok(Self {
            provider: Arc::new(Mutex::new(provider)),
        })
    }
}

/// Create the API router
pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/encrypt", post(encrypt_handler))
        .route("/decrypt", post(decrypt_handler))
        .with_state(state)
}

/// POST /encrypt - Encrypt plaintext with optional AAD
async fn encrypt_handler(
    State(state): State<AppState>,
    Json(req): Json<EncryptRequest>,
) -> Result<Json<EncryptResponse>, AppError> {
    // Decode base64 plaintext
    let plaintext = base64::decode(&req.plaintext_b64).map_err(|e| {
        AppError::BadRequest(format!("Invalid base64 plaintext: {}", e))
    })?;

    // Convert AAD to bytes if present
    let aad = req.aad.as_ref().map(|s| s.as_bytes());

    // Lock provider and encrypt
    let provider = state.provider.lock().unwrap();
    let result = provider.encrypt(&plaintext, aad).map_err(|e| {
        AppError::Internal(format!("Encryption failed: {}", e))
    })?;

    // Encode results to base64
    let response = EncryptResponse {
        keyver: 1, // MVP uses key version 1
        nonce_b64: base64::encode(&result.nonce),
        ciphertext_b64: base64::encode(&result.ciphertext),
        tag_b64: base64::encode(&result.tag),
    };

    Ok(Json(response))
}

/// POST /decrypt - Decrypt ciphertext with nonce and tag
async fn decrypt_handler(
    State(state): State<AppState>,
    Json(req): Json<DecryptRequest>,
) -> Result<Json<DecryptResponse>, AppError> {
    // Validate key version (MVP only supports version 1)
    if req.keyver != 1 {
        return Err(AppError::BadRequest(format!(
            "Unsupported key version: {}",
            req.keyver
        )));
    }

    // Decode base64 inputs
    let ciphertext = base64::decode(&req.ciphertext_b64).map_err(|e| {
        AppError::BadRequest(format!("Invalid base64 ciphertext: {}", e))
    })?;

    let nonce_vec = base64::decode(&req.nonce_b64).map_err(|e| {
        AppError::BadRequest(format!("Invalid base64 nonce: {}", e))
    })?;

    let tag_vec = base64::decode(&req.tag_b64).map_err(|e| {
        AppError::BadRequest(format!("Invalid base64 tag: {}", e))
    })?;

    // Validate nonce and tag sizes
    if nonce_vec.len() != CRYPTO_NONCE_SIZE {
        return Err(AppError::BadRequest(format!(
            "Invalid nonce size: expected {}, got {}",
            CRYPTO_NONCE_SIZE,
            nonce_vec.len()
        )));
    }

    if tag_vec.len() != CRYPTO_TAG_SIZE {
        return Err(AppError::BadRequest(format!(
            "Invalid tag size: expected {}, got {}",
            CRYPTO_TAG_SIZE,
            tag_vec.len()
        )));
    }

    // Convert to fixed-size arrays
    let mut nonce = [0u8; CRYPTO_NONCE_SIZE];
    let mut tag = [0u8; CRYPTO_TAG_SIZE];
    nonce.copy_from_slice(&nonce_vec);
    tag.copy_from_slice(&tag_vec);

    // Convert AAD to bytes if present
    let aad = req.aad.as_ref().map(|s| s.as_bytes());

    // Lock provider and decrypt
    let provider = state.provider.lock().unwrap();
    let plaintext = provider.decrypt(&ciphertext, &nonce, &tag, aad).map_err(|e| {
        AppError::CryptoError(format!("Decryption failed: {}", e))
    })?;

    // Encode plaintext to base64
    let response = DecryptResponse {
        plaintext_b64: base64::encode(&plaintext),
    };

    Ok(Json(response))
}

/// Application errors
#[derive(Debug)]
enum AppError {
    BadRequest(String),
    CryptoError(String),
    Internal(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            AppError::CryptoError(msg) => (StatusCode::UNPROCESSABLE_ENTITY, msg),
            AppError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
        };

        let body = Json(ErrorResponse::new(error_message));
        (status, body).into_response()
    }
}

// Helper function for base64 encoding/decoding
mod base64 {
    use base64::{engine::general_purpose::STANDARD, Engine as _};

    pub fn encode(data: &[u8]) -> String {
        STANDARD.encode(data)
    }

    pub fn decode(data: &str) -> Result<Vec<u8>, base64::DecodeError> {
        STANDARD.decode(data)
    }
}

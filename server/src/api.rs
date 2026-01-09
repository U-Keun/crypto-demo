use crate::crypto_ffi::{CryptoProvider, CRYPTO_KEY_SIZE, CRYPTO_NONCE_SIZE, CRYPTO_TAG_SIZE};
use crate::db::{Database, NewUser};
use crate::model::{
    CreateUserRequest, CreateUserResponse, DecryptRequest, DecryptResponse, EncryptRequest,
    EncryptResponse, ErrorResponse, GetUserResponse,
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use std::sync::{Arc, Mutex};

/// Application state shared across handlers
#[derive(Clone)]
pub struct AppState {
    pub provider: Arc<Mutex<CryptoProvider>>,
    pub db: Database,
}

impl AppState {
    /// Create new application state with a crypto provider and database
    pub fn new(key: &[u8; CRYPTO_KEY_SIZE], db: Database) -> Result<Self, String> {
        let provider = CryptoProvider::new(key).map_err(|e| e.to_string())?;
        Ok(Self {
            provider: Arc::new(Mutex::new(provider)),
            db,
        })
    }
}

/// Create the API router
pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/encrypt", post(encrypt_handler))
        .route("/decrypt", post(decrypt_handler))
        .route("/users", post(create_user_handler))
        .route("/users/:id", get(get_user_handler))
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

/// POST /users - Create a new user with encrypted phone
async fn create_user_handler(
    State(state): State<AppState>,
    Json(req): Json<CreateUserRequest>,
) -> Result<Json<CreateUserResponse>, AppError> {
    // Encrypt phone number
    let phone_bytes = req.phone.as_bytes();
    let aad = format!("table=users;field=phone;name={}", req.name);

    let provider = state.provider.lock().unwrap();
    let result = provider
        .encrypt(phone_bytes, Some(aad.as_bytes()))
        .map_err(|e| AppError::Internal(format!("Encryption failed: {}", e)))?;
    drop(provider);

    // Store in database
    let new_user = NewUser {
        name: req.name.clone(),
        phone_enc: result.ciphertext,
        phone_nonce: result.nonce.to_vec(),
        phone_tag: result.tag.to_vec(),
        phone_keyver: 1,
        phone_plaintext: phone_bytes.to_vec(),
    };

    let user_id = state
        .db
        .insert_user(new_user)
        .map_err(|e| AppError::Internal(format!("Database error: {}", e)))?;

    // Fetch the created user to get created_at
    let user = state
        .db
        .get_user(user_id)
        .map_err(|e| AppError::Internal(format!("Database error: {}", e)))?;

    Ok(Json(CreateUserResponse {
        id: user.id,
        name: user.name,
        created_at: user.created_at,
    }))
}

/// GET /users/{id} - Get user by ID with decrypted phone
async fn get_user_handler(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<GetUserResponse>, AppError> {
    // Fetch user from database
    let user = state
        .db
        .get_user(id)
        .map_err(|e| match e {
            crate::db::DbError::NotFound => AppError::NotFound(format!("User {} not found", id)),
            _ => AppError::Internal(format!("Database error: {}", e)),
        })?;

    // Decrypt phone number
    let aad = format!("table=users;field=phone;name={}", user.name);

    // Convert to fixed-size arrays
    if user.phone_nonce.len() != CRYPTO_NONCE_SIZE {
        return Err(AppError::Internal("Invalid nonce size in database".into()));
    }
    if user.phone_tag.len() != CRYPTO_TAG_SIZE {
        return Err(AppError::Internal("Invalid tag size in database".into()));
    }

    let mut nonce = [0u8; CRYPTO_NONCE_SIZE];
    let mut tag = [0u8; CRYPTO_TAG_SIZE];
    nonce.copy_from_slice(&user.phone_nonce);
    tag.copy_from_slice(&user.phone_tag);

    let provider = state.provider.lock().unwrap();
    let phone_bytes = provider
        .decrypt(&user.phone_enc, &nonce, &tag, Some(aad.as_bytes()))
        .map_err(|e| AppError::CryptoError(format!("Decryption failed: {}", e)))?;
    drop(provider);

    let phone = String::from_utf8(phone_bytes)
        .map_err(|e| AppError::Internal(format!("Invalid UTF-8 in decrypted phone: {}", e)))?;

    Ok(Json(GetUserResponse {
        id: user.id,
        name: user.name,
        phone,
        created_at: user.created_at,
    }))
}

/// Application errors
#[derive(Debug)]
enum AppError {
    BadRequest(String),
    NotFound(String),
    CryptoError(String),
    Internal(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
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

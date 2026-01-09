use std::ffi::CStr;
use std::os::raw::c_char;
use std::ptr;
use thiserror::Error;

// Constants from C header
pub const CRYPTO_NONCE_SIZE: usize = 12;
pub const CRYPTO_TAG_SIZE: usize = 16;
pub const CRYPTO_KEY_SIZE: usize = 32;

// Error type
#[derive(Debug, Error)]
pub enum CryptoError {
    #[error("Invalid argument")]
    InvalidArg,
    #[error("Key error")]
    Key,
    #[error("Cryptographic operation failed")]
    Crypto,
    #[error("Buffer too small")]
    BufferTooSmall,
    #[error("Internal error")]
    Internal,
    #[error("Unknown error code: {0}")]
    Unknown(i32),
}

impl From<i32> for CryptoError {
    fn from(code: i32) -> Self {
        match code {
            -1 => CryptoError::InvalidArg,
            -2 => CryptoError::Key,
            -3 => CryptoError::Crypto,
            -4 => CryptoError::BufferTooSmall,
            -5 => CryptoError::Internal,
            _ => CryptoError::Unknown(code),
        }
    }
}

// Opaque handle type
#[repr(C)]
struct CryptoProviderHandle {
    _private: [u8; 0],
}

// FFI declarations
unsafe extern "C" {
    fn crypto_provider_abi_version() -> i32;

    fn crypto_provider_create(key: *const u8, key_len: usize) -> *mut CryptoProviderHandle;

    fn crypto_provider_destroy(provider: *mut CryptoProviderHandle);

    fn crypto_provider_encrypt(
        provider: *mut CryptoProviderHandle,
        plaintext: *const u8,
        plaintext_len: usize,
        aad: *const u8,
        aad_len: usize,
        nonce: *mut u8,
        ciphertext: *mut u8,
        ciphertext_len: *mut usize,
        tag: *mut u8,
    ) -> i32;

    fn crypto_provider_decrypt(
        provider: *mut CryptoProviderHandle,
        ciphertext: *const u8,
        ciphertext_len: usize,
        aad: *const u8,
        aad_len: usize,
        nonce: *const u8,
        tag: *const u8,
        plaintext: *mut u8,
        plaintext_len: *mut usize,
    ) -> i32;

    fn crypto_provider_error_string(error_code: i32) -> *const c_char;
}

// Safe Rust wrapper
pub struct CryptoProvider {
    handle: *mut CryptoProviderHandle,
}

impl CryptoProvider {
    /// Get the ABI version
    pub fn abi_version() -> i32 {
        unsafe { crypto_provider_abi_version() }
    }

    /// Create a new CryptoProvider instance
    pub fn new(key: &[u8]) -> Result<Self, CryptoError> {
        if key.len() != CRYPTO_KEY_SIZE {
            return Err(CryptoError::Key);
        }

        let handle = unsafe { crypto_provider_create(key.as_ptr(), key.len()) };

        if handle.is_null() {
            return Err(CryptoError::Internal);
        }

        Ok(CryptoProvider { handle })
    }

    /// Encrypt plaintext with optional AAD
    pub fn encrypt(&self, plaintext: &[u8], aad: Option<&[u8]>) -> Result<EncryptResult, CryptoError> {
        let mut nonce = [0u8; CRYPTO_NONCE_SIZE];
        let mut ciphertext = vec![0u8; plaintext.len()];
        let mut ciphertext_len = ciphertext.len();
        let mut tag = [0u8; CRYPTO_TAG_SIZE];

        let (aad_ptr, aad_len) = match aad {
            Some(data) => (data.as_ptr(), data.len()),
            None => (ptr::null(), 0),
        };

        let ret = unsafe {
            crypto_provider_encrypt(
                self.handle,
                plaintext.as_ptr(),
                plaintext.len(),
                aad_ptr,
                aad_len,
                nonce.as_mut_ptr(),
                ciphertext.as_mut_ptr(),
                &mut ciphertext_len,
                tag.as_mut_ptr(),
            )
        };

        if ret != 0 {
            return Err(CryptoError::from(ret));
        }

        ciphertext.truncate(ciphertext_len);

        Ok(EncryptResult {
            nonce,
            ciphertext,
            tag,
        })
    }

    /// Decrypt ciphertext with nonce and tag
    pub fn decrypt(
        &self,
        ciphertext: &[u8],
        nonce: &[u8; CRYPTO_NONCE_SIZE],
        tag: &[u8; CRYPTO_TAG_SIZE],
        aad: Option<&[u8]>,
    ) -> Result<Vec<u8>, CryptoError> {
        let mut plaintext = vec![0u8; ciphertext.len()];
        let mut plaintext_len = plaintext.len();

        let (aad_ptr, aad_len) = match aad {
            Some(data) => (data.as_ptr(), data.len()),
            None => (ptr::null(), 0),
        };

        let ret = unsafe {
            crypto_provider_decrypt(
                self.handle,
                ciphertext.as_ptr(),
                ciphertext.len(),
                aad_ptr,
                aad_len,
                nonce.as_ptr(),
                tag.as_ptr(),
                plaintext.as_mut_ptr(),
                &mut plaintext_len,
            )
        };

        if ret != 0 {
            return Err(CryptoError::from(ret));
        }

        plaintext.truncate(plaintext_len);
        Ok(plaintext)
    }

    /// Get error string for error code
    pub fn error_string(error_code: i32) -> String {
        unsafe {
            let c_str = crypto_provider_error_string(error_code);
            if c_str.is_null() {
                return "Unknown error".to_string();
            }
            CStr::from_ptr(c_str)
                .to_string_lossy()
                .into_owned()
        }
    }
}

// RAII: ensure cleanup on drop
impl Drop for CryptoProvider {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe {
                crypto_provider_destroy(self.handle);
            }
        }
    }
}

// Safety: The CryptoProvider uses an opaque handle from C that manages its own state.
// Each encryption/decryption call is independent and the C provider implementation
// using OpenSSL EVP_CIPHER_CTX (which is created fresh for each operation) is thread-safe.
// The raw pointer is never dereferenced in Rust, only passed to C functions.
unsafe impl Send for CryptoProvider {}
unsafe impl Sync for CryptoProvider {}

/// Result of encryption operation
#[derive(Debug, Clone)]
pub struct EncryptResult {
    pub nonce: [u8; CRYPTO_NONCE_SIZE],
    pub ciphertext: Vec<u8>,
    pub tag: [u8; CRYPTO_TAG_SIZE],
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_key() -> [u8; CRYPTO_KEY_SIZE] {
        [0x42; CRYPTO_KEY_SIZE]
    }

    #[test]
    fn test_abi_version() {
        let version = CryptoProvider::abi_version();
        assert_eq!(version, 1);
    }

    #[test]
    fn test_create_provider() {
        let key = test_key();
        let provider = CryptoProvider::new(&key);
        assert!(provider.is_ok());
    }

    #[test]
    fn test_create_provider_invalid_key() {
        let key = [0u8; 16]; // Wrong size
        let provider = CryptoProvider::new(&key);
        assert!(matches!(provider, Err(CryptoError::Key)));
    }

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let key = test_key();
        let provider = CryptoProvider::new(&key).unwrap();

        let plaintext = b"Hello, World!";
        let result = provider.encrypt(plaintext, None).unwrap();

        let decrypted = provider
            .decrypt(&result.ciphertext, &result.nonce, &result.tag, None)
            .unwrap();

        assert_eq!(plaintext, &decrypted[..]);
    }

    #[test]
    fn test_encrypt_decrypt_with_aad() {
        let key = test_key();
        let provider = CryptoProvider::new(&key).unwrap();

        let plaintext = b"Secret data";
        let aad = b"table=users;field=phone;id=123";

        let result = provider.encrypt(plaintext, Some(aad)).unwrap();

        let decrypted = provider
            .decrypt(&result.ciphertext, &result.nonce, &result.tag, Some(aad))
            .unwrap();

        assert_eq!(plaintext, &decrypted[..]);
    }

    #[test]
    fn test_decrypt_with_wrong_tag_fails() {
        let key = test_key();
        let provider = CryptoProvider::new(&key).unwrap();

        let plaintext = b"Hello, World!";
        let mut result = provider.encrypt(plaintext, None).unwrap();

        // Tamper with tag
        result.tag[0] ^= 0xFF;

        let decrypted = provider.decrypt(&result.ciphertext, &result.nonce, &result.tag, None);

        assert!(matches!(decrypted, Err(CryptoError::Crypto)));
    }

    #[test]
    fn test_decrypt_with_wrong_aad_fails() {
        let key = test_key();
        let provider = CryptoProvider::new(&key).unwrap();

        let plaintext = b"Secret data";
        let aad = b"table=users;field=phone;id=123";
        let wrong_aad = b"table=users;field=phone;id=456";

        let result = provider.encrypt(plaintext, Some(aad)).unwrap();

        let decrypted = provider.decrypt(&result.ciphertext, &result.nonce, &result.tag, Some(wrong_aad));

        assert!(matches!(decrypted, Err(CryptoError::Crypto)));
    }
}

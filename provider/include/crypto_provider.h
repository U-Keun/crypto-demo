#ifndef CRYPTO_PROVIDER_H
#define CRYPTO_PROVIDER_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

// ABI Version
#define CRYPTO_PROVIDER_ABI_VERSION 1

// Constants
#define CRYPTO_NONCE_SIZE 12
#define CRYPTO_TAG_SIZE 16
#define CRYPTO_KEY_SIZE 32

// Error Codes (negative values indicate errors)
typedef enum {
    CRYPTO_OK = 0,
    CRYPTO_ERR_INVALID_ARG = -1,
    CRYPTO_ERR_KEY = -2,
    CRYPTO_ERR_CRYPTO = -3,
    CRYPTO_ERR_BUFFER_TOO_SMALL = -4,
    CRYPTO_ERR_INTERNAL = -5
} crypto_error_t;

// Opaque handle
typedef struct crypto_provider crypto_provider_t;

/**
 * Get ABI version
 */
int crypto_provider_abi_version(void);

/**
 * Create a new crypto provider instance
 * @param key: Encryption key (must be CRYPTO_KEY_SIZE bytes)
 * @param key_len: Length of key
 * @return: Provider handle or NULL on error
 */
crypto_provider_t* crypto_provider_create(const uint8_t* key, size_t key_len);

/**
 * Destroy a crypto provider instance
 * @param provider: Provider handle (can be NULL)
 */
void crypto_provider_destroy(crypto_provider_t* provider);

/**
 * Encrypt plaintext using AEAD (AES-256-GCM)
 * @param provider: Provider handle
 * @param plaintext: Input plaintext
 * @param plaintext_len: Length of plaintext
 * @param aad: Additional authenticated data (can be NULL if aad_len is 0)
 * @param aad_len: Length of AAD
 * @param nonce: Output buffer for nonce (must be CRYPTO_NONCE_SIZE bytes)
 * @param ciphertext: Output buffer for ciphertext (must be at least plaintext_len bytes)
 * @param ciphertext_len: Size of ciphertext buffer (in), actual bytes written (out)
 * @param tag: Output buffer for authentication tag (must be CRYPTO_TAG_SIZE bytes)
 * @return: CRYPTO_OK on success, error code on failure
 */
int crypto_provider_encrypt(
    crypto_provider_t* provider,
    const uint8_t* plaintext,
    size_t plaintext_len,
    const uint8_t* aad,
    size_t aad_len,
    uint8_t* nonce,
    uint8_t* ciphertext,
    size_t* ciphertext_len,
    uint8_t* tag
);

/**
 * Decrypt ciphertext using AEAD (AES-256-GCM)
 * @param provider: Provider handle
 * @param ciphertext: Input ciphertext
 * @param ciphertext_len: Length of ciphertext
 * @param aad: Additional authenticated data (must match encryption AAD)
 * @param aad_len: Length of AAD
 * @param nonce: Nonce used during encryption (must be CRYPTO_NONCE_SIZE bytes)
 * @param tag: Authentication tag from encryption (must be CRYPTO_TAG_SIZE bytes)
 * @param plaintext: Output buffer for plaintext (must be at least ciphertext_len bytes)
 * @param plaintext_len: Size of plaintext buffer (in), actual bytes written (out)
 * @return: CRYPTO_OK on success, error code on failure (including tag verification failure)
 */
int crypto_provider_decrypt(
    crypto_provider_t* provider,
    const uint8_t* ciphertext,
    size_t ciphertext_len,
    const uint8_t* aad,
    size_t aad_len,
    const uint8_t* nonce,
    const uint8_t* tag,
    uint8_t* plaintext,
    size_t* plaintext_len
);

/**
 * Get error string for error code
 * @param error_code: Error code
 * @return: Human-readable error string
 */
const char* crypto_provider_error_string(int error_code);

#ifdef __cplusplus
}
#endif

#endif // CRYPTO_PROVIDER_H

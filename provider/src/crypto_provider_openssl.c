#include "crypto_provider.h"
#include <openssl/evp.h>
#include <openssl/rand.h>
#include <openssl/err.h>
#include <string.h>
#include <stdlib.h>

// Opaque structure definition
struct crypto_provider {
    uint8_t key[CRYPTO_KEY_SIZE];
};

int crypto_provider_abi_version(void) {
    return CRYPTO_PROVIDER_ABI_VERSION;
}

crypto_provider_t* crypto_provider_create(const uint8_t* key, size_t key_len) {
    if (!key || key_len != CRYPTO_KEY_SIZE) {
        return NULL;
    }

    crypto_provider_t* provider = malloc(sizeof(crypto_provider_t));
    if (!provider) {
        return NULL;
    }

    memcpy(provider->key, key, CRYPTO_KEY_SIZE);
    return provider;
}

void crypto_provider_destroy(crypto_provider_t* provider) {
    if (provider) {
        // Clear sensitive data
        memset(provider->key, 0, CRYPTO_KEY_SIZE);
        free(provider);
    }
}

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
) {
    if (!provider || !plaintext || !nonce || !ciphertext || !ciphertext_len || !tag) {
        return CRYPTO_ERR_INVALID_ARG;
    }

    if (aad_len > 0 && !aad) {
        return CRYPTO_ERR_INVALID_ARG;
    }

    if (*ciphertext_len < plaintext_len) {
        return CRYPTO_ERR_BUFFER_TOO_SMALL;
    }

    EVP_CIPHER_CTX* ctx = NULL;
    int len = 0;
    int ret = CRYPTO_ERR_INTERNAL;

    // Generate random nonce
    if (RAND_bytes(nonce, CRYPTO_NONCE_SIZE) != 1) {
        goto cleanup;
    }

    // Create and initialize the context
    ctx = EVP_CIPHER_CTX_new();
    if (!ctx) {
        goto cleanup;
    }

    // Initialize encryption with AES-256-GCM
    if (EVP_EncryptInit_ex(ctx, EVP_aes_256_gcm(), NULL, NULL, NULL) != 1) {
        goto cleanup;
    }

    // Set nonce length (12 bytes is default for GCM, but being explicit)
    if (EVP_CIPHER_CTX_ctrl(ctx, EVP_CTRL_GCM_SET_IVLEN, CRYPTO_NONCE_SIZE, NULL) != 1) {
        goto cleanup;
    }

    // Initialize key and nonce
    if (EVP_EncryptInit_ex(ctx, NULL, NULL, provider->key, nonce) != 1) {
        goto cleanup;
    }

    // Provide AAD if present
    if (aad_len > 0) {
        if (EVP_EncryptUpdate(ctx, NULL, &len, aad, aad_len) != 1) {
            goto cleanup;
        }
    }

    // Encrypt plaintext
    if (EVP_EncryptUpdate(ctx, ciphertext, &len, plaintext, plaintext_len) != 1) {
        goto cleanup;
    }
    *ciphertext_len = len;

    // Finalize encryption
    if (EVP_EncryptFinal_ex(ctx, ciphertext + len, &len) != 1) {
        goto cleanup;
    }
    *ciphertext_len += len;

    // Get the tag
    if (EVP_CIPHER_CTX_ctrl(ctx, EVP_CTRL_GCM_GET_TAG, CRYPTO_TAG_SIZE, tag) != 1) {
        goto cleanup;
    }

    ret = CRYPTO_OK;

cleanup:
    if (ctx) {
        EVP_CIPHER_CTX_free(ctx);
    }
    return ret;
}

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
) {
    if (!provider || !ciphertext || !nonce || !tag || !plaintext || !plaintext_len) {
        return CRYPTO_ERR_INVALID_ARG;
    }

    if (aad_len > 0 && !aad) {
        return CRYPTO_ERR_INVALID_ARG;
    }

    if (*plaintext_len < ciphertext_len) {
        return CRYPTO_ERR_BUFFER_TOO_SMALL;
    }

    EVP_CIPHER_CTX* ctx = NULL;
    int len = 0;
    int ret = CRYPTO_ERR_CRYPTO;

    // Create and initialize the context
    ctx = EVP_CIPHER_CTX_new();
    if (!ctx) {
        ret = CRYPTO_ERR_INTERNAL;
        goto cleanup;
    }

    // Initialize decryption with AES-256-GCM
    if (EVP_DecryptInit_ex(ctx, EVP_aes_256_gcm(), NULL, NULL, NULL) != 1) {
        ret = CRYPTO_ERR_INTERNAL;
        goto cleanup;
    }

    // Set nonce length
    if (EVP_CIPHER_CTX_ctrl(ctx, EVP_CTRL_GCM_SET_IVLEN, CRYPTO_NONCE_SIZE, NULL) != 1) {
        ret = CRYPTO_ERR_INTERNAL;
        goto cleanup;
    }

    // Initialize key and nonce
    if (EVP_DecryptInit_ex(ctx, NULL, NULL, provider->key, nonce) != 1) {
        ret = CRYPTO_ERR_INTERNAL;
        goto cleanup;
    }

    // Provide AAD if present
    if (aad_len > 0) {
        if (EVP_DecryptUpdate(ctx, NULL, &len, aad, aad_len) != 1) {
            goto cleanup;
        }
    }

    // Decrypt ciphertext
    if (EVP_DecryptUpdate(ctx, plaintext, &len, ciphertext, ciphertext_len) != 1) {
        goto cleanup;
    }
    *plaintext_len = len;

    // Set expected tag value
    if (EVP_CIPHER_CTX_ctrl(ctx, EVP_CTRL_GCM_SET_TAG, CRYPTO_TAG_SIZE, (void*)tag) != 1) {
        ret = CRYPTO_ERR_INTERNAL;
        goto cleanup;
    }

    // Finalize decryption (this verifies the tag)
    int final_ret = EVP_DecryptFinal_ex(ctx, plaintext + len, &len);
    if (final_ret > 0) {
        *plaintext_len += len;
        ret = CRYPTO_OK;
    } else {
        // Tag verification failed or other decryption error
        ret = CRYPTO_ERR_CRYPTO;
    }

cleanup:
    if (ctx) {
        EVP_CIPHER_CTX_free(ctx);
    }
    return ret;
}

const char* crypto_provider_error_string(int error_code) {
    switch (error_code) {
        case CRYPTO_OK:
            return "Success";
        case CRYPTO_ERR_INVALID_ARG:
            return "Invalid argument";
        case CRYPTO_ERR_KEY:
            return "Key error";
        case CRYPTO_ERR_CRYPTO:
            return "Cryptographic operation failed";
        case CRYPTO_ERR_BUFFER_TOO_SMALL:
            return "Buffer too small";
        case CRYPTO_ERR_INTERNAL:
            return "Internal error";
        default:
            return "Unknown error";
    }
}

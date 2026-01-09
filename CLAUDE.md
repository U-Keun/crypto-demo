# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 1. Project Summary

**Purpose**: Build a pluggable vendor crypto module (provider) integrated with a Rust API server and SQLite DB column encryption demo.

**Key Value Propositions**:
- C-based provider (shared library) design with ABI stability (versioning + error codes)
- Rust FFI with safe wrapper layers
- AEAD-based encryption/decryption with integrity (AAD context binding)
- DB column encryption storage + separated search tokens (exact match queries)

## 2. Repository Structure

```
crypto-demo/
  provider/
    include/crypto_provider.h
    src/crypto_provider_openssl.c
    CMakeLists.txt
    build/                    # cmake build output (gitignored)
  server/
    Cargo.toml
    build.rs
    src/
      main.rs
      crypto_ffi.rs           # extern "C" + safe wrapper
      api.rs                  # routes
      db.rs                   # sqlite operations
      model.rs                # request/response DTOs
  docs/
    ARCHITECTURE.md
    THREAT_MODEL.md
    BENCHMARK.md
  README.md
  CLAUDE.md
```

## 3. Build & Run (macOS)

### 3.1 Build Provider (CMake)

From repository root:

```bash
cd provider
mkdir -p build
cd build
cmake ..
cmake --build . -j
cd ../../
```

Expected artifact: `provider/build/libcrypto_provider.dylib`

### 3.2 Run Rust Server with dylib Path

From repository root:

```bash
export DYLD_LIBRARY_PATH="$(pwd)/provider/build:$DYLD_LIBRARY_PATH"
cargo run --manifest-path server/Cargo.toml
```

### 3.3 Alternative: Copy dylib Next to Executable

```bash
cargo build --manifest-path server/Cargo.toml
cp provider/build/libcrypto_provider.dylib server/target/debug/
./server/target/debug/server
```

## 4. Provider API Contract (C)

The provider acts as a "vendor module" with ABI stability guarantees.

**ABI Stability Requirements**:
- Fixed `CRYPTO_PROVIDER_ABI_VERSION`
- Opaque handle pattern (`crypto_provider_t*`)
- Error code enum (negative values for errors)

**AEAD Format**:
- Nonce: 12 bytes (recommended)
- Tag: 16 bytes (returned separately)
- Ciphertext: same length as plaintext (no padding) + tag separated

**AAD (Additional Authenticated Data)**:
- Used for context binding (string/bytes)
- Example: `table=users;field=phone;id=123;keyver=1`

**Error Code Rules**:
- Argument errors: `CRYPTO_ERR_INVALID_ARG`
- Key errors/length mismatch: `CRYPTO_ERR_KEY`
- Crypto failures (tag mismatch, etc.): `CRYPTO_ERR_CRYPTO`
- Buffer too small: `CRYPTO_ERR_BUFFER_TOO_SMALL`
- Other errors: `CRYPTO_ERR_INTERNAL`

## 5. Rust FFI & Safety Rules

Rust should minimize `extern "C"` declarations and only expose safe wrappers externally.

**Memory/Buffer Management**:
- Pointers passed to C functions must remain valid for the duration of the call
- Output buffers are pre-allocated in Rust; C functions populate them
- Never pass Rust `String` or `Vec` directly without proper pointer/length handling

**Thread-Safety Policy**:
- Initial MVP: use `Arc<Mutex<CryptoProvider>>` for serialization (safety first)
- If provider implementation is proven re-entrant (each call uses independent context), can add `Sync` and remove locks

## 6. API MVP (Rust)

### Endpoints

**POST /encrypt**
```json
Input:  { "aad": "...", "plaintext_b64": "..." }
Output: { "keyver": 1, "nonce_b64": "...", "ciphertext_b64": "...", "tag_b64": "..." }
```

**POST /decrypt**
```json
Input:  { "aad": "...", "keyver": 1, "nonce_b64": "...", "ciphertext_b64": "...", "tag_b64": "..." }
Output: { "plaintext_b64": "..." }
```

**POST /users**
```json
Input: { "name": "...", "phone": "..." }
```
Stores encrypted phone + token

**GET /users/{id}**
Returns decrypted phone (MVP uses simple auth token check)

## 7. DB Schema (SQLite)

### MVP Table

```sql
CREATE TABLE users (
  id INTEGER PRIMARY KEY,
  name TEXT,
  phone_enc BLOB,
  phone_nonce BLOB,
  phone_tag BLOB,
  phone_keyver INTEGER,
  phone_token BLOB,
  created_at TEXT
);

CREATE INDEX idx_users_phone_token ON users(phone_token);
```

### Token Policy

- `phone_token = HMAC(master_key, phone_plain)` for exact match searches
- Partial searches/LIKE queries are NOT supported in MVP (document this limitation)

## 8. Testing & Benchmarks (Must-Have)

### Tests

- **Round-trip**: encrypt → decrypt yields original plaintext
- **Tamper detection**: modifying tag/nonce/ciphertext causes decrypt failure
- **Invalid input**: key length errors, missing fields, base64 decode errors

### Benchmarks

- **Payload sizes**: 32B / 1KB / 64KB / 1MB
- **Metrics**: p50/p95 latency, TPS (transactions per second)
- **Results**: Record as tables in `docs/BENCHMARK.md`

## 9. Documentation Checklist

- **docs/ARCHITECTURE.md**: provider↔FFI↔API↔DB flow, data formats, AAD rules
- **docs/THREAT_MODEL.md**: MVP scope, protection targets, attack assumptions, unsupported scenarios
- **README.md**: Quickstart (3 lines), demo scenarios, example curl commands, sample outputs

## 10. Roadmap (Recommended)

1. Provider build + Rust FFI smoke test (encrypt/decrypt)
2. `/encrypt` `/decrypt` endpoints
3. SQLite storage (`/users`, `/users/{id}`)
4. Token separation and search queries (exact match)
5. Concurrency/performance refinement (benchmarks + docs)
6. (Optional) dlopen-based dynamic loading for provider swapping

## 11. Conventions

- **C Code**: C11 standard, apply clang-format where possible
- **Rust Code**: Use rustfmt, minimize clippy warnings
- **Commit Messages**: Use prefixes: `feat:`, `fix:`, `docs:`, `chore:`
- **Priority**: Focus on "integration/operations/compatibility" rather than "algorithm implementation"

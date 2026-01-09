# Architecture

이 문서는 crypto-demo 프로젝트의 아키텍처와 데이터 흐름을 설명합니다.

## 전체 구조

```
┌─────────────────────────────────────────────────────────────┐
│                         Client (HTTP)                        │
└─────────────────────────┬───────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────────┐
│                     Axum REST API                            │
│  (server/src/api.rs)                                         │
│  - POST /encrypt, POST /decrypt                              │
│  - POST /users, GET /users/{id}                              │
└────────────┬──────────────────────────┬─────────────────────┘
             │                          │
             ▼                          ▼
┌─────────────────────────┐  ┌──────────────────────────────┐
│  CryptoProvider (Rust)  │  │    Database (SQLite)         │
│  (server/src/           │  │    (server/src/db.rs)        │
│   crypto_ffi.rs)        │  │                              │
│  - Safe FFI wrapper     │  │  - users table               │
│  - Arc<Mutex<>>         │  │  - HMAC token generation     │
└────────────┬────────────┘  └──────────────────────────────┘
             │
             ▼
┌─────────────────────────────────────────────────────────────┐
│              C Provider (Dynamic Library)                    │
│  (provider/src/crypto_provider_openssl.c)                    │
│                                                              │
│  - AES-256-GCM (OpenSSL EVP API)                            │
│  - Opaque handle (crypto_provider_t*)                       │
│  - ABI versioning                                           │
└────────────┬────────────────────────────────────────────────┘
             │
             ▼
┌─────────────────────────────────────────────────────────────┐
│                    OpenSSL Library                           │
└─────────────────────────────────────────────────────────────┘
```

## 계층별 책임

### 1. API Layer (api.rs)

**책임**:
- HTTP 요청/응답 처리
- JSON 직렬화/역직렬화
- Base64 인코딩/디코딩
- 에러 핸들링 (400, 404, 422, 500)
- 상태 관리 (AppState)

**주요 엔드포인트**:

```rust
POST /encrypt
  Input:  { aad?, plaintext_b64 }
  Output: { keyver, nonce_b64, ciphertext_b64, tag_b64 }

POST /decrypt
  Input:  { aad?, keyver, nonce_b64, ciphertext_b64, tag_b64 }
  Output: { plaintext_b64 }

POST /users
  Input:  { name, phone }
  Output: { id, name, created_at }

GET /users/{id}
  Output: { id, name, phone, created_at }
```

### 2. FFI Layer (crypto_ffi.rs)

**책임**:
- C 함수 바인딩 (`extern "C"`)
- 안전한 Rust 래퍼 제공
- 메모리 관리 (RAII 패턴)
- 버퍼 할당 및 검증
- 에러 변환 (C int → Rust Result)

**안전성 보장**:
- 포인터 유효성 검증
- 버퍼 크기 사전 할당
- Drop trait으로 자동 cleanup
- Send + Sync 구현 (thread-safe)

### 3. C Provider Layer (crypto_provider_openssl.c)

**책임**:
- AES-256-GCM 암호화/복호화
- 난수 생성 (nonce)
- OpenSSL API 호출
- 에러 코드 반환

**ABI 안정성**:
- 고정된 ABI 버전 (v1)
- Opaque handle (구조체 세부사항 숨김)
- 명시적 에러 코드
- C ABI 호환 함수 시그니처

### 4. Database Layer (db.rs)

**책임**:
- SQLite 연결 관리
- 스키마 초기화
- HMAC 토큰 생성
- CRUD 연산

**스키마**:
```sql
CREATE TABLE users (
  id INTEGER PRIMARY KEY,
  name TEXT NOT NULL,
  phone_enc BLOB NOT NULL,      -- 암호화된 전화번호
  phone_nonce BLOB NOT NULL,    -- 12 bytes
  phone_tag BLOB NOT NULL,      -- 16 bytes
  phone_keyver INTEGER NOT NULL,
  phone_token BLOB NOT NULL,    -- HMAC for search
  created_at TEXT NOT NULL
);

CREATE INDEX idx_users_phone_token ON users(phone_token);
```

## 데이터 흐름

### 사용자 생성 흐름 (POST /users)

```
1. Client → API
   POST /users { "name": "Alice", "phone": "010-1234-5678" }

2. API → CryptoProvider
   encrypt("010-1234-5678", aad="table=users;field=phone;name=Alice")

3. CryptoProvider → C Provider
   crypto_provider_encrypt(provider, plaintext, aad, ...)

4. C Provider → OpenSSL
   EVP_EncryptInit_ex(ctx, EVP_aes_256_gcm(), ...)
   EVP_EncryptUpdate(ctx, ciphertext, plaintext)
   EVP_CIPHER_CTX_ctrl(ctx, EVP_CTRL_GCM_GET_TAG, tag)

5. Result: { nonce[12], ciphertext[14], tag[16] }

6. API → Database
   HMAC(key, "010-1234-5678") → phone_token
   INSERT INTO users(name, phone_enc, phone_nonce, phone_tag, phone_token, ...)

7. Database → API
   user_id = 1

8. API → Client
   { "id": 1, "name": "Alice", "created_at": "..." }
```

### 사용자 조회 흐름 (GET /users/{id})

```
1. Client → API
   GET /users/1

2. API → Database
   SELECT * FROM users WHERE id = 1

3. Database → API
   { id, name, phone_enc, phone_nonce, phone_tag, ... }

4. API → CryptoProvider
   decrypt(phone_enc, nonce, tag, aad="table=users;field=phone;name=Alice")

5. CryptoProvider → C Provider
   crypto_provider_decrypt(provider, ciphertext, nonce, tag, aad, ...)

6. C Provider → OpenSSL
   EVP_DecryptInit_ex(ctx, EVP_aes_256_gcm(), ...)
   EVP_CIPHER_CTX_ctrl(ctx, EVP_CTRL_GCM_SET_TAG, tag)
   EVP_DecryptUpdate(ctx, plaintext, ciphertext)
   EVP_DecryptFinal_ex(ctx, ...) // tag 검증

7. Result: "010-1234-5678"

8. API → Client
   { "id": 1, "name": "Alice", "phone": "010-1234-5678", "created_at": "..." }
```

## AEAD 데이터 포맷

### 암호화 출력

```
┌─────────────┬──────────────────┬─────────────┐
│   Nonce     │   Ciphertext     │     Tag     │
│  12 bytes   │   N bytes        │  16 bytes   │
└─────────────┴──────────────────┴─────────────┘

- Nonce: 랜덤 생성 (RAND_bytes)
- Ciphertext: AES-GCM 암호문 (평문과 동일 길이)
- Tag: GCM 인증 태그 (무결성 검증)
```

### AAD (Additional Authenticated Data)

AAD는 암호화되지 않지만 인증되는 데이터입니다. 변조 시 복호화 실패.

**형식**:
```
table={table_name};field={field_name};name={context}
```

**예시**:
```
table=users;field=phone;name=Alice
table=users;field=email;name=Bob
```

**목적**:
- 컨텍스트 바인딩 (암호문이 다른 컨텍스트에서 사용되는 것 방지)
- 재생 공격 방지
- 필드 간 암호문 교환 방지

## 키 관리

### MVP 구현

```rust
// 하드코딩된 키 (테스트/데모 전용)
let encryption_key = [0x42; 32];  // 암호화 키
let hmac_key = [0x42; 32];        // 검색 토큰 키
```

### 프로덕션 권장사항

1. **환경 변수로 주입**
   ```bash
   export CRYPTO_KEY=$(openssl rand -hex 32)
   export HMAC_KEY=$(openssl rand -hex 32)
   ```

2. **키 관리 시스템 (KMS)**
   - AWS KMS
   - HashiCorp Vault
   - Azure Key Vault

3. **키 회전**
   - `phone_keyver` 컬럼으로 버전 추적
   - 주기적인 키 교체
   - 이전 버전 키 보관 (복호화용)

## 동시성 모델

### Thread-Safety

**CryptoProvider**:
```rust
// C provider는 각 호출마다 독립적인 EVP_CIPHER_CTX 사용
// → Re-entrant 함수
unsafe impl Send for CryptoProvider {}
unsafe impl Sync for CryptoProvider {}
```

**AppState**:
```rust
pub struct AppState {
    pub provider: Arc<Mutex<CryptoProvider>>,  // 직렬화
    pub db: Database,                          // Arc<Mutex<Connection>>
}
```

**최적화 옵션**:
- Provider가 re-entrant임이 검증되면 Mutex 제거 가능
- 읽기 전용 작업은 Arc만으로 충분

## 빌드 프로세스

```
1. CMake로 C provider 빌드
   provider/build/libcrypto_provider.dylib 생성

2. build.rs가 링크 경로 설정
   cargo:rustc-link-search=native=../provider/build
   cargo:rustc-link-lib=dylib=crypto_provider

3. cargo build로 Rust 서버 빌드
   FFI를 통해 C provider 링크

4. 런타임에 dylib 로드
   DYLD_LIBRARY_PATH 환경변수 필요 (macOS)
```

## 에러 처리 전략

### 계층별 에러 변환

```
C Provider (int)
    ↓
FFI Layer (Result<T, CryptoError>)
    ↓
API Layer (Result<Json, AppError>)
    ↓
HTTP Response (StatusCode + JSON)
```

**예시**:
```rust
// C: CRYPTO_ERR_CRYPTO (-3)
//   ↓
// FFI: CryptoError::Crypto
//   ↓
// API: AppError::CryptoError("Decryption failed: ...")
//   ↓
// HTTP: 422 Unprocessable Entity
```

## 확장 가능성

### Provider 교체

현재 구조는 provider를 쉽게 교체할 수 있도록 설계되어 있습니다:

1. **정적 링크** (현재):
   - build.rs에서 링크
   - 컴파일 타임에 결정

2. **동적 로딩** (향후):
   ```rust
   use libloading::Library;

   let lib = Library::new("libcrypto_provider.dylib")?;
   let create_fn: Symbol<CreateFn> = lib.get(b"crypto_provider_create")?;
   ```

### 지원 가능한 Provider 예시

- OpenSSL (현재)
- BoringSSL
- LibSodium
- Hardware Security Module (HSM)
- Cloud KMS wrapper

각 provider는 동일한 ABI를 준수하면 됩니다.

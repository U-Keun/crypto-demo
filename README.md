# Crypto Demo

교체 가능한 벤더 암호화 모듈(C provider)과 Rust API 서버를 통합한 SQLite DB 컬럼 암호화 데모 프로젝트입니다.

## 주요 특징

- **C 기반 Provider**: OpenSSL을 사용한 AES-256-GCM AEAD 암호화
- **ABI 안정성**: 버전 관리, opaque handle 패턴, 명시적 에러 코드
- **Rust FFI**: 안전한 래퍼 계층으로 C 라이브러리 호출
- **AAD 컨텍스트 바인딩**: 추가 인증 데이터를 통한 암호화 컨텍스트 검증
- **DB 컬럼 암호화**: SQLite에 암호화된 데이터 저장 + 검색 토큰 분리 (예정)

## Quickstart

```bash
# 1. C provider 빌드
cd provider/build && cmake .. && cmake --build . -j && cd ../..

# 2. Rust 서버 실행
export DYLD_LIBRARY_PATH="$(pwd)/provider/build:$DYLD_LIBRARY_PATH"
cargo run

# 3. 테스트 실행
cargo test
```

## 시스템 요구사항

- **macOS**: 개발 및 테스트 환경 (Apple Silicon / Intel 모두 지원)
- **OpenSSL**: Homebrew를 통해 설치 (`brew install openssl`)
- **CMake**: 3.16 이상
- **Rust**: Edition 2024 (최신 stable 권장)

## 프로젝트 구조

```
crypto-demo/
├── provider/              # C 암호화 라이브러리
│   ├── include/           # 공개 헤더
│   │   └── crypto_provider.h
│   ├── src/               # OpenSSL 구현
│   │   └── crypto_provider_openssl.c
│   └── build/             # 빌드 산출물 (gitignored)
│       └── libcrypto_provider.dylib
├── server/                # Rust API 서버
│   ├── src/
│   │   ├── main.rs        # 엔트리 포인트 + 스모크 테스트
│   │   └── crypto_ffi.rs  # FFI 바인딩 + 안전한 래퍼
│   └── build.rs           # 링크 설정
└── docs/                  # 문서
    ├── ARCHITECTURE.md
    ├── THREAT_MODEL.md
    └── BENCHMARK.md
```

## 빌드 상세

### 1. C Provider 빌드

```bash
cd provider
mkdir -p build
cd build
cmake ..
cmake --build . -j
```

빌드 결과: `provider/build/libcrypto_provider.dylib` (약 34KB)

### 2. Rust 서버 빌드

```bash
# dylib 경로 설정 후 빌드
export DYLD_LIBRARY_PATH="$(pwd)/provider/build:$DYLD_LIBRARY_PATH"
cargo build
```

### 3. 테스트 실행

```bash
# 단위 테스트 (FFI, 암복호화, 변조 감지)
export DYLD_LIBRARY_PATH="$(pwd)/provider/build:$DYLD_LIBRARY_PATH"
cargo test

# 실행 결과:
# - test_abi_version
# - test_encrypt_decrypt_roundtrip
# - test_encrypt_decrypt_with_aad
# - test_decrypt_with_wrong_tag_fails
# - test_decrypt_with_wrong_aad_fails
```

## 현재 구현 상태

### ✅ 완료 (Phase 1)

- [x] C provider 헤더 정의 (`crypto_provider.h`)
- [x] OpenSSL 기반 AES-256-GCM 구현 (`crypto_provider_openssl.c`)
- [x] CMake 빌드 시스템
- [x] Rust FFI 안전 래퍼 (`crypto_ffi.rs`)
- [x] 기본 테스트 스위트 (7개 테스트)
- [x] 스모크 테스트 (encrypt → decrypt 라운드트립)

### 🚧 진행 예정 (Phase 2)

- [ ] Axum 기반 REST API 서버
- [ ] `/encrypt` 엔드포인트
- [ ] `/decrypt` 엔드포인트
- [ ] Base64 인코딩 DTO

### 📋 계획 (Phase 3+)

- [ ] SQLite DB 연동
- [ ] `/users` CRUD 엔드포인트
- [ ] 컬럼 암호화 저장
- [ ] HMAC 기반 검색 토큰
- [ ] 성능 벤치마크
- [ ] 동시성 최적화

## Provider API 개요

### 주요 함수

```c
// Provider 생성/해제
crypto_provider_t* crypto_provider_create(const uint8_t* key, size_t key_len);
void crypto_provider_destroy(crypto_provider_t* provider);

// 암호화 (AES-256-GCM)
int crypto_provider_encrypt(
    crypto_provider_t* provider,
    const uint8_t* plaintext, size_t plaintext_len,
    const uint8_t* aad, size_t aad_len,
    uint8_t* nonce,           // 출력: 12 bytes
    uint8_t* ciphertext,      // 출력: plaintext_len bytes
    size_t* ciphertext_len,
    uint8_t* tag              // 출력: 16 bytes
);

// 복호화
int crypto_provider_decrypt(
    crypto_provider_t* provider,
    const uint8_t* ciphertext, size_t ciphertext_len,
    const uint8_t* aad, size_t aad_len,
    const uint8_t* nonce,     // 12 bytes
    const uint8_t* tag,       // 16 bytes
    uint8_t* plaintext,       // 출력
    size_t* plaintext_len
);
```

### 에러 코드

| 코드 | 의미 | 발생 조건 |
|------|------|-----------|
| `CRYPTO_OK` (0) | 성공 | - |
| `CRYPTO_ERR_INVALID_ARG` (-1) | 인자 오류 | NULL 포인터, 잘못된 길이 |
| `CRYPTO_ERR_KEY` (-2) | 키 오류 | 키 길이 != 32 bytes |
| `CRYPTO_ERR_CRYPTO` (-3) | 암복호화 실패 | 태그 불일치, AAD 불일치 |
| `CRYPTO_ERR_BUFFER_TOO_SMALL` (-4) | 버퍼 부족 | 출력 버퍼 크기 부족 |
| `CRYPTO_ERR_INTERNAL` (-5) | 내부 오류 | OpenSSL 초기화 실패 |

### AEAD 포맷

```
Nonce:      12 bytes (랜덤 생성)
Tag:        16 bytes (GCM 인증 태그)
Ciphertext: N bytes (평문과 동일 길이, 패딩 없음)
```

### AAD 예시

```
table=users;field=phone;id=123;keyver=1
```

## 실행 예제

```bash
$ export DYLD_LIBRARY_PATH="$(pwd)/provider/build:$DYLD_LIBRARY_PATH"
$ cargo run

Crypto Provider Demo
====================
ABI Version: 1
✓ Provider created successfully

Encryption Test:
  Plaintext: "Hello, Crypto World!"
✓ Encryption successful
  Nonce length: 12
  Ciphertext length: 20
  Tag length: 16

Decryption Test:
✓ Decryption successful
  Decrypted: "Hello, Crypto World!"
✓ Round-trip successful: plaintext matches!

✓ All smoke tests passed!
```

## 테스트 결과

```bash
$ cargo test

running 7 tests
test crypto_ffi::tests::test_abi_version ... ok
test crypto_ffi::tests::test_create_provider ... ok
test crypto_ffi::tests::test_create_provider_invalid_key ... ok
test crypto_ffi::tests::test_decrypt_with_wrong_tag_fails ... ok
test crypto_ffi::tests::test_decrypt_with_wrong_aad_fails ... ok
test crypto_ffi::tests::test_encrypt_decrypt_roundtrip ... ok
test crypto_ffi::tests::test_encrypt_decrypt_with_aad ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured
```

## 개발 가이드

상세한 개발 가이드는 [CLAUDE.md](CLAUDE.md)를 참조하세요.

### 커밋 컨벤션

- `feat:` - 새로운 기능
- `fix:` - 버그 수정
- `docs:` - 문서 변경
- `chore:` - 빌드, 설정 변경

### 코드 포맷팅

```bash
# Rust
cargo fmt
cargo clippy

# C (선택사항)
clang-format -i provider/src/*.c provider/include/*.h
```

## 문제 해결

### dylib를 찾을 수 없음

```bash
# 환경변수 설정 확인
export DYLD_LIBRARY_PATH="$(pwd)/provider/build:$DYLD_LIBRARY_PATH"

# 또는 dylib 복사
cp provider/build/libcrypto_provider.dylib server/target/debug/
```

### OpenSSL을 찾을 수 없음

```bash
brew install openssl
cd provider/build
rm -rf *
cmake ..
cmake --build . -j
```

### Rust 빌드 오류

```bash
# C provider가 먼저 빌드되었는지 확인
ls -lh provider/build/libcrypto_provider.dylib

# 클린 빌드
cargo clean
cargo build
```

## 라이선스

Educational/Demo purposes only.

## 참고 문서

- [CLAUDE.md](CLAUDE.md) - 개발 가이드 (Claude Code용)
- [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) - 아키텍처 설계 (예정)
- [docs/THREAT_MODEL.md](docs/THREAT_MODEL.md) - 위협 모델 (예정)
- [docs/BENCHMARK.md](docs/BENCHMARK.md) - 성능 벤치마크 (예정)

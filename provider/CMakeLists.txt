cmake_minimum_required(VERSION 3.16)
project(crypto_provider C)

set(CMAKE_C_STANDARD 11)
set(CMAKE_POSITION_INDEPENDENT_CODE ON)
set(CMAKE_MACOSX_RPATH ON)

add_library(crypto_provider SHARED
    src/crypto_provider_openssl.c
)

target_include_directories(crypto_provider PUBLIC
    ${CMAKE_CURRENT_SOURCE_DIR}/include
)

# OpenSSL (Homebrew 기준)
find_package(OpenSSL REQUIRED)
target_link_libraries(crypto_provider PRIVATE OpenSSL::Crypto)

# dylib install name을 @rpath 기반으로
set_target_properties(crypto_provider PROPERTIES
    OUTPUT_NAME "crypto_provider"
    INSTALL_NAME_DIR "@rpath"
)

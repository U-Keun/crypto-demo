mod crypto_ffi;

use crypto_ffi::{CryptoProvider, CRYPTO_KEY_SIZE};

fn main() {
    println!("Crypto Provider Demo");
    println!("====================");

    // Test ABI version
    let version = CryptoProvider::abi_version();
    println!("ABI Version: {}", version);

    // Create a test key
    let key = [0x42u8; CRYPTO_KEY_SIZE];

    // Create provider
    let provider = match CryptoProvider::new(&key) {
        Ok(p) => {
            println!("✓ Provider created successfully");
            p
        }
        Err(e) => {
            eprintln!("✗ Failed to create provider: {}", e);
            std::process::exit(1);
        }
    };

    // Test encryption
    let plaintext = b"Hello, Crypto World!";
    let aad = b"table=test;field=data;id=1";

    println!("\nEncryption Test:");
    println!("  Plaintext: {:?}", String::from_utf8_lossy(plaintext));

    let result = match provider.encrypt(plaintext, Some(aad)) {
        Ok(r) => {
            println!("✓ Encryption successful");
            println!("  Nonce length: {}", r.nonce.len());
            println!("  Ciphertext length: {}", r.ciphertext.len());
            println!("  Tag length: {}", r.tag.len());
            r
        }
        Err(e) => {
            eprintln!("✗ Encryption failed: {}", e);
            std::process::exit(1);
        }
    };

    // Test decryption
    println!("\nDecryption Test:");
    match provider.decrypt(&result.ciphertext, &result.nonce, &result.tag, Some(aad)) {
        Ok(decrypted) => {
            println!("✓ Decryption successful");
            println!("  Decrypted: {:?}", String::from_utf8_lossy(&decrypted));

            if decrypted == plaintext {
                println!("✓ Round-trip successful: plaintext matches!");
            } else {
                eprintln!("✗ Round-trip failed: plaintext mismatch");
                std::process::exit(1);
            }
        }
        Err(e) => {
            eprintln!("✗ Decryption failed: {}", e);
            std::process::exit(1);
        }
    }

    println!("\n✓ All smoke tests passed!");
}

use std::env;
use std::path::PathBuf;

fn main() {
    // Get the workspace root (project root)
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let workspace_root = PathBuf::from(manifest_dir).parent().unwrap().to_path_buf();
    let provider_build = workspace_root.join("provider").join("build");

    println!("cargo:rustc-link-search=native={}", provider_build.display());
    println!("cargo:rustc-link-lib=dylib=crypto_provider");

    // Re-run if the dylib changes
    println!("cargo:rerun-if-changed={}", provider_build.join("libcrypto_provider.dylib").display());
}

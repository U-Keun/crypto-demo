mod api;
mod crypto_ffi;
mod db;
mod model;

use api::{create_router, AppState};
use crypto_ffi::CRYPTO_KEY_SIZE;
use db::Database;
use std::net::SocketAddr;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "server=debug,tower_http=debug,axum=trace".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Create encryption key (MVP: hardcoded key, production should use key management)
    let key = [0x42u8; CRYPTO_KEY_SIZE];

    // Create HMAC key for search tokens (MVP: same as encryption key)
    let hmac_key = key;

    // Initialize database
    let db = Database::new("crypto_demo.db", &hmac_key).expect("Failed to initialize database");
    tracing::info!("Database initialized: crypto_demo.db");

    // Initialize application state
    let state = AppState::new(&key, db).expect("Failed to create application state");

    // Create router
    let app = create_router(state);

    // Server address
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::info!("Starting server on {}", addr);

    // Run server
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("Failed to bind to address");

    tracing::info!("Server listening on {}", addr);
    tracing::info!("Endpoints:");
    tracing::info!("  POST http://{}/encrypt", addr);
    tracing::info!("  POST http://{}/decrypt", addr);
    tracing::info!("  POST http://{}/users", addr);
    tracing::info!("  GET  http://{}/users/{{id}}", addr);

    axum::serve(listener, app)
        .await
        .expect("Server error");
}

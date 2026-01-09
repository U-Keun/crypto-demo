mod api;
mod crypto_ffi;
mod model;

use api::{create_router, AppState};
use crypto_ffi::CRYPTO_KEY_SIZE;
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

    // Initialize application state
    let state = AppState::new(&key).expect("Failed to create application state");

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

    axum::serve(listener, app)
        .await
        .expect("Server error");
}

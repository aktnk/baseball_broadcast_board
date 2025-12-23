pub mod state;
pub mod protocol;
pub mod handlers;
pub mod persistence;

pub use state::{AppState, ClientRole};
pub use protocol::Message;

use axum::{Router, routing::get};
use tower_http::services::ServeDir;
use std::sync::Arc;
use std::path::PathBuf;

/// Get the base directory for resources (public/, config/, data/)
fn get_resource_dir() -> PathBuf {
    #[cfg(debug_assertions)]
    {
        // Development: use current directory
        PathBuf::from(".")
    }
    #[cfg(not(debug_assertions))]
    {
        // Production: resources are in _up_ directory relative to executable
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.join("_up_")))
            .unwrap_or_else(|| PathBuf::from("_up_"))
    }
}

pub async fn start_server(state: Arc<AppState>, port: u16) -> Result<(), Box<dyn std::error::Error>> {
    let base_dir = get_resource_dir();
    let public_dir = base_dir.join("public");

    println!("Starting server on port {}...", port);
    println!("Resolved resource dir: {:?}", base_dir);
    println!("Serving static files from: {:?}", public_dir);
    println!("Public dir exists: {}", public_dir.exists());

    // Check if public directory exists
    if !public_dir.exists() {
        eprintln!("ERROR: Public directory not found at: {:?}", public_dir);
        eprintln!("Current working directory: {:?}", std::env::current_dir()?);
        eprintln!("Current executable: {:?}", std::env::current_exe()?);
        return Err("Public directory not found".into());
    }

    // Load persisted game state if it exists
    persistence::load_game_state(&state).await?;

    let app = Router::new()
        .route("/ws", get(handlers::websocket::websocket_handler))
        .route("/init_data.json", get(handlers::init_data::init_data_handler))
        .nest_service("/", ServeDir::new(public_dir))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", port))
        .await?;

    println!("Server running on http://127.0.0.1:{}", port);

    axum::serve(listener, app)
        .await?;

    Ok(())
}
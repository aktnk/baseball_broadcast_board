#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod server;

use std::sync::Arc;
use tauri::Manager;

#[tokio::main]
async fn main() {
    // Start HTTP + WebSocket server
    // Always start server when running as standalone binary
    // Only skip when running via `npm run tauri:dev` (NODE_ENV=development)
    let should_start_server = std::env::var("NODE_ENV").unwrap_or_default() != "development";

    if should_start_server {
        // Initialize server state
        let app_state = Arc::new(server::AppState::new());

        // Start HTTP + WebSocket server in background
        let server_state = app_state.clone();
        tokio::spawn(async move {
            if let Err(e) = server::start_server(server_state, 8080).await {
                eprintln!("Server error: {}", e);
            }
        });

        // Wait for server to be ready
        println!("Waiting for server to start...");
        tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;

        // Verify server is accessible
        for i in 1..=5 {
            match reqwest::get("http://localhost:8080").await {
                Ok(_) => {
                    println!("Server is ready!");
                    break;
                }
                Err(_) if i < 5 => {
                    println!("Waiting for server... (attempt {}/5)", i);
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                }
                Err(e) => {
                    eprintln!("Failed to connect to server: {}", e);
                    std::process::exit(1);
                }
            }
        }
    } else {
        println!("Development mode: Using external Node.js server");
    }

    // Build and run Tauri application
    tauri::Builder::default()
        .setup(|app| {
            // Optional: Open dev tools in debug mode
            #[cfg(debug_assertions)]
            {
                let window = app.get_webview_window("operation").unwrap();
                window.open_devtools();
            }

            Ok(())
        })
        .on_window_event(|_window, event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                println!("Window closing, server will shutdown gracefully");
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
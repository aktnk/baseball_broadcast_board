use axum::{
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use std::path::PathBuf;
use tokio::fs;

/// Get the config directory path
fn get_config_dir() -> PathBuf {
    #[cfg(debug_assertions)]
    {
        PathBuf::from("./config")
    }
    #[cfg(not(debug_assertions))]
    {
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.join("_up_").join("config")))
            .unwrap_or_else(|| PathBuf::from("./config"))
    }
}

/// Handler for /init_data.json endpoint
/// Serves the init_data.json file from the config directory
pub async fn init_data_handler() -> impl IntoResponse {
    let config_dir = get_config_dir();
    let init_data_path = config_dir.join("init_data.json");

    match fs::read_to_string(&init_data_path).await {
        Ok(content) => {
            // Parse and return as JSON to validate it's proper JSON
            match serde_json::from_str::<serde_json::Value>(&content) {
                Ok(json) => (StatusCode::OK, Json(json)).into_response(),
                Err(e) => {
                    eprintln!("Failed to parse init_data.json: {}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!("Invalid JSON in init_data.json: {}", e),
                    )
                        .into_response()
                }
            }
        }
        Err(e) => {
            eprintln!("Failed to read init_data.json from {:?}: {}", init_data_path, e);
            (
                StatusCode::NOT_FOUND,
                format!("init_data.json not found at {:?}", init_data_path),
            )
                .into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_config_dir() {
        let config_dir = get_config_dir();
        #[cfg(debug_assertions)]
        assert_eq!(config_dir, PathBuf::from("./config"));
    }
}
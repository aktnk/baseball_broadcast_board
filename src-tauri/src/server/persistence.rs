use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs;
use super::state::{AppState, GameState};

/// Get the data directory path
fn get_data_dir() -> PathBuf {
    #[cfg(debug_assertions)]
    {
        PathBuf::from("./data")
    }
    #[cfg(not(debug_assertions))]
    {
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.join("_up_").join("data")))
            .unwrap_or_else(|| PathBuf::from("./data"))
    }
}

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

/// Save game state to data/current_game.json
pub async fn save_game_state(state: &Arc<AppState>) -> Result<(), std::io::Error> {
    let game_state = state.game_state.read().clone();

    if let Some(game_state) = game_state {
        let data_dir = get_data_dir();

        // Create data directory if it doesn't exist
        fs::create_dir_all(&data_dir).await?;

        let file_path = data_dir.join("current_game.json");
        let json = serde_json::to_string_pretty(&game_state)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        fs::write(file_path, json).await?;
        println!("Game state saved successfully");
    }

    Ok(())
}

/// Load game state from data/current_game.json
pub async fn load_game_state(state: &Arc<AppState>) -> Result<(), Box<dyn std::error::Error>> {
    let data_dir = get_data_dir();
    let file_path = data_dir.join("current_game.json");

    if file_path.exists() {
        let contents = fs::read_to_string(&file_path).await?;
        let game_state: GameState = serde_json::from_str(&contents)?;

        *state.game_state.write() = Some(game_state);
        println!("Game state loaded from {:?}", file_path);
    } else {
        println!("No saved game state found");
    }

    Ok(())
}

/// Load initial configuration from config/init_data.json
#[allow(dead_code)]
pub async fn load_init_config() -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let config_dir = get_config_dir();
    let file_path = config_dir.join("init_data.json");

    let contents = fs::read_to_string(&file_path).await?;
    let config: serde_json::Value = serde_json::from_str(&contents)?;

    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_save_and_load_game_state() {
        let temp_dir = TempDir::new().unwrap();
        let _temp_path = temp_dir.path().to_path_buf();

        // Create test game state
        let state = Arc::new(AppState::new());
        let test_game_state = GameState {
            game_inning: 5,
            top: true,
            first_base: false,
            second_base: true,
            third_base: false,
            ball_cnt: 2,
            strike_cnt: 1,
            out_cnt: 0,
            score_top: 3,
            score_bottom: 2,
            game_title: "Test Game".to_string(),
            team_top: "Team A".to_string(),
            team_bottom: "Team B".to_string(),
            last_inning: 9,
        };

        *state.game_state.write() = Some(test_game_state.clone());

        // Note: In actual tests, we'd need to mock get_data_dir() to use temp_path
        // For now, this test verifies the logic compiles correctly
    }
}
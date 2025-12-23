use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::atomic::AtomicU64;
use uuid::Uuid;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::UnboundedSender;

pub type ClientId = u64;
pub type MasterToken = String;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ClientRole {
    Master,
    Slave,
    Viewer,
}

#[derive(Debug, Clone)]
pub struct ClientInfo {
    pub id: ClientId,
    pub role: ClientRole,
    pub client_type: String,
    pub connected_at: DateTime<Utc>,
    pub sender: UnboundedSender<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GameState {
    pub game_inning: i32,
    pub top: bool,
    pub first_base: bool,
    pub second_base: bool,
    pub third_base: bool,
    pub ball_cnt: i32,
    pub strike_cnt: i32,
    pub out_cnt: i32,
    pub score_top: i32,
    pub score_bottom: i32,
    pub game_title: String,
    pub team_top: String,
    pub team_bottom: String,
    pub last_inning: i32,
}

#[derive(Debug, Clone)]
pub struct TokenGracePeriod {
    pub token: MasterToken,
    pub expires_at: DateTime<Utc>,
}

pub struct AppState {
    pub clients: RwLock<HashMap<ClientId, ClientInfo>>,
    pub game_state: RwLock<Option<GameState>>,
    pub master_client_id: RwLock<Option<ClientId>>,
    pub master_token: RwLock<Option<MasterToken>>,
    pub master_token_grace: RwLock<Option<TokenGracePeriod>>,
    pub client_counter: AtomicU64,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            clients: RwLock::new(HashMap::new()),
            game_state: RwLock::new(None),
            master_client_id: RwLock::new(None),
            master_token: RwLock::new(None),
            master_token_grace: RwLock::new(None),
            client_counter: AtomicU64::new(1),
        }
    }

    pub fn generate_master_token() -> MasterToken {
        Uuid::new_v4().to_string()
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_state_initialization() {
        let state = AppState::new();
        assert!(state.clients.read().is_empty());
        assert!(state.game_state.read().is_none());
        assert!(state.master_client_id.read().is_none());
        assert!(state.master_token.read().is_none());
        assert_eq!(state.client_counter.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[test]
    fn test_generate_master_token() {
        let token1 = AppState::generate_master_token();
        let token2 = AppState::generate_master_token();

        // Tokens should be non-empty
        assert!(!token1.is_empty());
        assert!(!token2.is_empty());

        // Tokens should be unique
        assert_ne!(token1, token2);

        // Token should be valid UUID format
        assert!(Uuid::parse_str(&token1).is_ok());
    }

    #[test]
    fn test_client_role_serialization() {
        let master = ClientRole::Master;
        let slave = ClientRole::Slave;
        let viewer = ClientRole::Viewer;

        let master_json = serde_json::to_string(&master).unwrap();
        let slave_json = serde_json::to_string(&slave).unwrap();
        let viewer_json = serde_json::to_string(&viewer).unwrap();

        assert_eq!(master_json, "\"Master\"");
        assert_eq!(slave_json, "\"Slave\"");
        assert_eq!(viewer_json, "\"Viewer\"");
    }

    #[test]
    fn test_game_state_serialization() {
        let game_state = GameState {
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
            game_title: "夏季大会".to_string(),
            team_top: "横浜M".to_string(),
            team_bottom: "静岡D".to_string(),
            last_inning: 9,
        };

        let json = serde_json::to_string(&game_state).unwrap();
        let deserialized: GameState = serde_json::from_str(&json).unwrap();

        assert_eq!(game_state, deserialized);
    }
}
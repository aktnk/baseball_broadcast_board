use serde::{Deserialize, Serialize};
use super::state::{ClientRole, GameState, MasterToken};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Message {
    #[serde(rename = "handshake")]
    Handshake {
        #[serde(rename = "clientType")]
        client_type: String,
        #[serde(rename = "masterToken")]
        master_token: Option<MasterToken>,
    },

    #[serde(rename = "role_assignment")]
    RoleAssignment {
        role: ClientRole,
        #[serde(rename = "clientId")]
        client_id: u64,
        #[serde(rename = "masterClientId")]
        master_client_id: Option<u64>,
        #[serde(rename = "masterToken")]
        master_token: Option<MasterToken>,
    },

    #[serde(rename = "role_changed")]
    RoleChanged {
        #[serde(rename = "newRole")]
        new_role: ClientRole,
        #[serde(rename = "clientId")]
        client_id: u64,
        #[serde(rename = "masterClientId")]
        master_client_id: Option<u64>,
        #[serde(rename = "masterToken")]
        master_token: Option<MasterToken>,
        #[serde(rename = "clearToken")]
        clear_token: Option<bool>,
    },

    #[serde(rename = "game_state_update")]
    GameStateUpdate {
        #[serde(rename = "boardData")]
        board_data: GameState,
    },

    #[serde(rename = "game_state")]
    GameStateBroadcast {
        #[serde(rename = "boardData")]
        board_data: GameState,
    },

    #[serde(rename = "release_master")]
    ReleaseMaster,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handshake_message_serialization() {
        let msg = Message::Handshake {
            client_type: "operation".to_string(),
            master_token: Some("test-token-123".to_string()),
        };

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"handshake\""));
        assert!(json.contains("\"clientType\":\"operation\""));
        assert!(json.contains("\"masterToken\":\"test-token-123\""));
    }

    #[test]
    fn test_handshake_message_deserialization() {
        let json = r#"{
            "type": "handshake",
            "clientType": "operation",
            "masterToken": "test-token-123"
        }"#;

        let msg: Message = serde_json::from_str(json).unwrap();
        match msg {
            Message::Handshake { client_type, master_token } => {
                assert_eq!(client_type, "operation");
                assert_eq!(master_token, Some("test-token-123".to_string()));
            }
            _ => panic!("Expected Handshake message"),
        }
    }

    #[test]
    fn test_role_assignment_message() {
        let msg = Message::RoleAssignment {
            role: ClientRole::Master,
            client_id: 123,
            master_client_id: Some(123),
            master_token: Some("master-token".to_string()),
        };

        let json = serde_json::to_string(&msg).unwrap();
        let deserialized: Message = serde_json::from_str(&json).unwrap();

        match deserialized {
            Message::RoleAssignment { role, client_id, .. } => {
                assert_eq!(role, ClientRole::Master);
                assert_eq!(client_id, 123);
            }
            _ => panic!("Expected RoleAssignment message"),
        }
    }

    #[test]
    fn test_game_state_update_message() {
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
            game_title: "Test Game".to_string(),
            team_top: "Team A".to_string(),
            team_bottom: "Team B".to_string(),
            last_inning: 9,
        };

        let msg = Message::GameStateUpdate {
            board_data: game_state.clone(),
        };

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"game_state_update\""));
        assert!(json.contains("\"boardData\""));

        let deserialized: Message = serde_json::from_str(&json).unwrap();
        match deserialized {
            Message::GameStateUpdate { board_data } => {
                assert_eq!(board_data, game_state);
            }
            _ => panic!("Expected GameStateUpdate message"),
        }
    }

    #[test]
    fn test_release_master_message() {
        let msg = Message::ReleaseMaster;
        let json = serde_json::to_string(&msg).unwrap();
        assert_eq!(json, "{\"type\":\"release_master\"}");

        let deserialized: Message = serde_json::from_str(&json).unwrap();
        assert!(matches!(deserialized, Message::ReleaseMaster));
    }
}
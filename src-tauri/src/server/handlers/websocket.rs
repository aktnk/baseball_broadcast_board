use axum::{
    extract::{ws::{WebSocket, Message as WsMessage}, State, WebSocketUpgrade},
    response::Response,
};
use futures::{SinkExt, StreamExt};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use chrono::{Utc, Duration};

use crate::server::{AppState, Message, ClientRole};
use crate::server::state::{ClientInfo, TokenGracePeriod};
use crate::server::persistence;

pub async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> Response {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: Arc<AppState>) {
    let (mut sender, mut receiver) = socket.split();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

    let client_id = state.client_counter.fetch_add(1, Ordering::SeqCst);
    println!("New WebSocket connection: client_id={}", client_id);

    // Spawn task to send messages from channel to WebSocket
    let send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if sender.send(WsMessage::Text(msg)).await.is_err() {
                break;
            }
        }
    });

    // Handle incoming messages
    while let Some(msg) = receiver.next().await {
        if let Ok(WsMessage::Text(text)) = msg {
            if let Ok(message) = serde_json::from_str::<Message>(&text) {
                handle_message(client_id, message, &state, &tx).await;
            } else {
                eprintln!("Failed to parse message from client {}: {}", client_id, text);
            }
        }
    }

    // Cleanup on disconnect
    println!("Client {} disconnected", client_id);
    handle_disconnect(client_id, &state).await;
    send_task.abort();
}

async fn handle_message(
    client_id: u64,
    message: Message,
    state: &Arc<AppState>,
    tx: &tokio::sync::mpsc::UnboundedSender<String>,
) {
    match message {
        Message::Handshake { client_type, master_token } => {
            handle_handshake(client_id, client_type, master_token, state, tx).await;
        }
        Message::GameStateUpdate { board_data } => {
            handle_game_state_update(client_id, board_data, state).await;
        }
        Message::ReleaseMaster => {
            handle_release_master(client_id, state).await;
        }
        _ => {
            eprintln!("Unexpected message type from client {}", client_id);
        }
    }
}

async fn handle_handshake(
    client_id: u64,
    client_type: String,
    master_token: Option<String>,
    state: &Arc<AppState>,
    tx: &tokio::sync::mpsc::UnboundedSender<String>,
) {
    println!("Handshake from client {} (type: {})", client_id, client_type);

    // Determine client role
    let role = if client_type == "operation" {
        determine_operation_role(client_id, master_token, state).await
    } else {
        ClientRole::Viewer
    };

    // Store client info
    let client_info = ClientInfo {
        id: client_id,
        role: role.clone(),
        client_type: client_type.clone(),
        connected_at: Utc::now(),
        sender: tx.clone(),
    };

    state.clients.write().insert(client_id, client_info);

    // Send role assignment
    let master_client_id = state.master_client_id.read().clone();
    let response_token = if role == ClientRole::Master {
        state.master_token.read().clone()
    } else {
        None
    };

    let response = Message::RoleAssignment {
        role: role.clone(),
        client_id,
        master_client_id,
        master_token: response_token,
    };

    if let Ok(json) = serde_json::to_string(&response) {
        let _ = tx.send(json);
    }

    // Send current game state to new client
    if let Some(game_state) = state.game_state.read().clone() {
        let state_msg = Message::GameStateBroadcast {
            board_data: game_state,
        };
        if let Ok(json) = serde_json::to_string(&state_msg) {
            let _ = tx.send(json);
        }
    }

    println!("Client {} assigned role: {:?}", client_id, role);
}

async fn determine_operation_role(
    client_id: u64,
    provided_token: Option<String>,
    state: &Arc<AppState>,
) -> ClientRole {
    // Check if there's a grace period token and it matches
    if let Some(provided_token) = &provided_token {
        let grace_period = state.master_token_grace.read().clone();
        if let Some(grace) = grace_period {
            if &grace.token == provided_token && Utc::now() < grace.expires_at {
                // Restore master role within grace period
                println!("Restoring master role for reconnection (token: {})", provided_token);
                *state.master_client_id.write() = Some(client_id);
                *state.master_token.write() = Some(provided_token.clone());
                *state.master_token_grace.write() = None; // Clear grace period
                return ClientRole::Master;
            }
        }
    }

    // Check if there's already a master
    let current_master = state.master_client_id.read().clone();
    if current_master.is_none() {
        // No master exists, assign this client as master
        let new_token = AppState::generate_master_token();
        *state.master_client_id.write() = Some(client_id);
        *state.master_token.write() = Some(new_token);
        *state.master_token_grace.write() = None;
        ClientRole::Master
    } else {
        // Master exists, assign as slave
        ClientRole::Slave
    }
}

async fn handle_game_state_update(
    client_id: u64,
    board_data: crate::server::state::GameState,
    state: &Arc<AppState>,
) {
    // Verify client is master
    let master_id = state.master_client_id.read().clone();
    if master_id != Some(client_id) {
        eprintln!("Client {} attempted to update game state but is not master", client_id);
        return;
    }

    println!("Updating game state from client {}", client_id);

    // Update game state
    *state.game_state.write() = Some(board_data.clone());

    // Save to disk asynchronously
    let state_clone = state.clone();
    tokio::spawn(async move {
        if let Err(e) = persistence::save_game_state(&state_clone).await {
            eprintln!("Failed to save game state: {}", e);
        }
    });

    // Broadcast to all clients
    broadcast_game_state(state, board_data).await;
}

async fn broadcast_game_state(
    state: &Arc<AppState>,
    board_data: crate::server::state::GameState,
) {
    let message = Message::GameStateBroadcast { board_data };

    if let Ok(json) = serde_json::to_string(&message) {
        let clients = state.clients.read();
        let client_count = clients.len();
        println!("Broadcasting game state to {} clients", client_count);

        for (id, client_info) in clients.iter() {
            println!("  Sending to client {} (role: {:?})", id, client_info.role);
            if client_info.sender.send(json.clone()).is_err() {
                eprintln!("Failed to send game state to client {}", id);
            } else {
                println!("  Successfully sent to client {}", id);
            }
        }
    }
}

async fn handle_release_master(client_id: u64, state: &Arc<AppState>) {
    // Verify client is master
    let master_id = state.master_client_id.read().clone();
    if master_id != Some(client_id) {
        eprintln!("Client {} attempted to release master but is not master", client_id);
        return;
    }

    println!("Client {} releasing master authority", client_id);

    // Clear master
    *state.master_client_id.write() = None;
    *state.master_token.write() = None;
    *state.master_token_grace.write() = None;

    // Change former master to slave (exclude from promotion)
    {
        let mut clients = state.clients.write();
        if let Some(client_info) = clients.get_mut(&client_id) {
            client_info.role = ClientRole::Slave;
        }
    }

    // Promote next slave to master
    promote_next_slave(state).await;
}

async fn handle_disconnect(client_id: u64, state: &Arc<AppState>) {
    // Remove client from clients map
    let removed_client = state.clients.write().remove(&client_id);

    if let Some(client_info) = removed_client {
        println!("Removed client {} (role: {:?})", client_id, client_info.role);

        // Check if disconnected client was master
        let master_id = state.master_client_id.read().clone();
        if master_id == Some(client_id) {
            println!("Master client {} disconnected, starting grace period", client_id);

            // Start 5-second grace period
            let old_token = state.master_token.read().clone();
            if let Some(token) = old_token {
                let grace_period = TokenGracePeriod {
                    token,
                    expires_at: Utc::now() + Duration::seconds(5),
                };
                *state.master_token_grace.write() = Some(grace_period);

                // Schedule promotion after grace period
                let state_clone = state.clone();
                tokio::spawn(async move {
                    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                    promote_if_grace_expired(&state_clone).await;
                });
            }
        }
    }
}

async fn promote_if_grace_expired(state: &Arc<AppState>) {
    // Check if grace period has expired
    let grace_period = state.master_token_grace.read().clone();
    if let Some(grace) = grace_period {
        if Utc::now() >= grace.expires_at {
            println!("Grace period expired, promoting next slave");
            *state.master_token_grace.write() = None;
            promote_next_slave(state).await;
        }
    }
}

async fn promote_next_slave(state: &Arc<AppState>) {
    let clients = state.clients.read();

    // Find the oldest slave (operation type)
    let oldest_slave = clients
        .values()
        .filter(|c| c.role == ClientRole::Slave && c.client_type == "operation")
        .min_by_key(|c| c.connected_at);

    if let Some(slave) = oldest_slave {
        let new_master_id = slave.id;
        let new_token = AppState::generate_master_token();

        println!("Promoting slave {} to master", new_master_id);

        // Update state
        drop(clients); // Release read lock
        *state.master_client_id.write() = Some(new_master_id);
        *state.master_token.write() = Some(new_token.clone());

        // Update client role
        {
            let mut clients = state.clients.write();
            if let Some(client_info) = clients.get_mut(&new_master_id) {
                client_info.role = ClientRole::Master;

                // Send role changed message
                let message = Message::RoleChanged {
                    new_role: ClientRole::Master,
                    client_id: new_master_id,
                    master_client_id: Some(new_master_id),
                    master_token: Some(new_token),
                    clear_token: None,
                };

                if let Ok(json) = serde_json::to_string(&message) {
                    let _ = client_info.sender.send(json);
                }
            }
        }
    } else {
        println!("No slaves available for promotion");
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_websocket_handler_compiles() {
        // This test just ensures the module compiles correctly
        // Actual WebSocket testing would require integration tests
        assert!(true);
    }
}

use crate::models::db::Attachment;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct ChatMessage {
    pub to: String,
    pub message_type: String,
    pub from: Option<String>,
    pub content: String,
    #[serde(default)]
    pub attachments: Vec<Attachment>,
}

#[derive(Serialize, Deserialize)]
pub struct ServerMessage {
    pub message_type: String,
    pub content: String,
}

#[derive(Deserialize)]
pub struct AuthMessage {
    pub token: String,
}

#[derive(Deserialize)]
pub struct RefreshMessage {
    pub refresh_token: String,
}

#[derive(Serialize)]
pub struct PresenceEvent {
    pub message_type: String,
    pub user_id: String,
}

#[derive(Deserialize)]
pub struct VoiceJoinMessage {
    pub channel_id: String,
}

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct ChatMessage {
    pub to: String,
    pub from: Option<String>,
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

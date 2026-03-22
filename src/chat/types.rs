use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct ChatMessage {
    pub to: String,
    pub from: Option<String>,
    pub content: String,
}

#[derive(serde::Deserialize)]
pub struct AuthMessage {
    pub token: String,
}

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct ChatMessage {
    pub to: String,
    pub from: String,
    pub content: String,
}

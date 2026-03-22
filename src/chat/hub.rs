use rocket::tokio::sync::RwLock;
use rocket::tokio::sync::broadcast::Sender;
use std::collections::HashMap;

pub struct ChatHub {
    pub connections: RwLock<HashMap<String, Sender<String>>>,
}

impl ChatHub {
    pub fn new() -> Self {
        ChatHub {
            connections: RwLock::new(HashMap::new()),
        }
    }
}

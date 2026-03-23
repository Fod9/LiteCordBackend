use rocket::tokio::sync::RwLock;
use rocket::tokio::sync::broadcast::Sender;
use std::collections::HashMap;
use surrealdb::Surreal;
use surrealdb::engine::remote::ws::Client;
use surrealdb::sql::Thing;

use crate::chat::types::ChatMessage;

pub struct ChatHub {
    pub connections: RwLock<HashMap<String, Sender<String>>>,
}

impl ChatHub {
    pub fn new() -> Self {
        ChatHub {
            connections: RwLock::new(HashMap::new()),
        }
    }

    pub async fn send_to_dm_channel(db: &Surreal<Client>, message: &ChatMessage) {}

    pub async fn send_to_channel(db: &Surreal<Client>, message: &ChatMessage) {}

    pub async fn send_to(hub: &ChatHub, db: &Surreal<Client>, message: &ChatMessage) {
        let target = &message.to;
        let splitted_target: Vec<&str> = target.split(":").collect();
        if let Some(id) = splitted_target.get(1) {
            let data_type = splitted_target.get(0).unwrap();
            match *data_type {
                "user" => println!("This message should be sent to a user"), // CREATE A DM_CHANNEL
                "dm_channel" => println!("This message should be sent to a dm_channel"), // SEND TO DM CHANNEL
                "channel" => println!("This message should be send to a channel"), // SEND TO CHANNEL
                &_ => println!("Unknown data type {data_type:}"),
            }
        }

        let get_recipients_result = db
            .query("SELECT recipients FROM DMChannel WHERE DMChannel.id = $channel_id")
            .bind(("channel_id", target.clone()))
            .await;

        if let Ok(mut recipients_result) = get_recipients_result {
            let recipients_vector: Vec<String> = recipients_result.take(0).unwrap_or_default();
            dbg!(recipients_vector);
        }
    }
}

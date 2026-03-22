use std::sync::Arc;

use crate::chat::types::ChatMessage;
use rocket::futures::{SinkExt, StreamExt};
use rocket::get;
use rocket::serde::json::serde_json;
use rocket::tokio::sync::broadcast;
use rocket::{State, tokio};
use rocket_ws::{self, Channel, Message, WebSocket};

use crate::chat::hub::ChatHub;

#[get("/<user_id>")]
pub fn websocket_index(
    ws: WebSocket,
    user_id: String,
    hub: &State<Arc<ChatHub>>,
) -> Channel<'static> {
    let hub = hub.inner().clone(); // clone l'Arc AVANT le closure

    ws.channel(move |mut stream| {
        Box::pin(async move {
            let (tx, mut rx) = broadcast::channel(100);
            {
                let mut conns = hub.connections.write().await;
                conns.insert(user_id.clone(), tx);
            }
            loop {
                tokio::select! {
                    Some(msg) = stream.next() => {
                        match msg? {
                            Message::Text(text) => {
                                if let Ok(chat_msg) = serde_json::from_str::<ChatMessage>(&text) {
                                    let conns = hub.connections.read().await;
                                    if let Some(target_tx) = conns.get(&chat_msg.to) {
                                        let _ = target_tx.send(text);
                                    }
                                }
                            }
                            Message::Close(_) => break,
                            _ => {}
                        }
                    }
                    Ok(msg) = rx.recv() => {
                        stream.send(Message::Text(msg)).await?;
                    }
                }
            }
            let mut conns = hub.connections.write().await;
            conns.remove(&user_id);

            Ok(())
        })
    })
}

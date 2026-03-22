use crate::chat::hub::ChatHub;
use crate::chat::types::{AuthMessage, ChatMessage};
use crate::jwt::decode_token;
use rocket::futures::{SinkExt, StreamExt};
use rocket::get;
use rocket::serde::json::serde_json;
use rocket::tokio::sync::broadcast;
use rocket::{State, tokio};
use rocket_ws::{self, Channel, Message, WebSocket};
use std::sync::Arc;

#[get("/")]
pub fn websocket_index(ws: WebSocket, hub: &State<Arc<ChatHub>>) -> Channel<'static> {
    let hub = hub.inner().clone();
    ws.channel(move |mut stream| {
        Box::pin(async move {
            let user_id = loop {
                match stream.next().await {
                    Some(Ok(Message::Text(text))) => {
                        match serde_json::from_str::<AuthMessage>(&text) {
                            Ok(auth_msg) => {
                                if let Ok(claims) = decode_token(&auth_msg.token) {
                                    break claims.user_id;
                                } else {
                                    let err = r#"{"error":"invalid token"}"#;
                                    stream.send(Message::Text(err.into())).await?;
                                    return Ok(());
                                }
                            }
                            Err(_) => {
                                let err = r#"{"error":"expected auth message"}"#;
                                stream.send(Message::Text(err.into())).await?;
                            }
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => return Ok(()),
                    _ => {}
                }
            };

            let (tx, mut rx) = broadcast::channel(100);
            {
                let mut conns = hub.connections.write().await;
                conns.insert(user_id.clone(), tx);
                println!("User {} connected", user_id);
            }

            let ok = r#"{"status":"authenticated"}"#;
            stream.send(Message::Text(ok.into())).await?;

            loop {
                tokio::select! {
                    Some(msg) = stream.next() => {
                        match msg? {
                            Message::Text(text) => {
                                match serde_json::from_str::<ChatMessage>(&text) {
                                    Ok(mut chat_msg) => {
                                        chat_msg.from = Some(user_id.clone());
                                        let conns = hub.connections.read().await;
                                        if let Some(target_tx) = conns.get(&chat_msg.to) {
                                            let json = serde_json::to_string(&chat_msg).unwrap();
                                            let _ = target_tx.send(json);
                                        }
                                    }
                                    Err(e) => println!("Parse error: {e}"),
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

            // 5. Cleanup
            let mut conns = hub.connections.write().await;
            conns.remove(&user_id);
            println!("User {} disconnected", user_id);
            Ok(())
        })
    })
}

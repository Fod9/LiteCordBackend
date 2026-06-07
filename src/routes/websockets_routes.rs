use crate::chat::hub::ChatHub;
use crate::chat::types::{AuthMessage, ChatMessage, RefreshMessage};
use crate::jwt::decode_token;
use crate::users::auth::refresh_token;
use rocket::futures::{SinkExt, StreamExt};
use rocket::get;
use rocket::serde::json::serde_json;
use rocket::tokio::sync::broadcast;
use rocket::tokio::time::{Duration, interval};
use rocket::{State, tokio};
use rocket_ws::stream::DuplexStream;
use rocket_ws::{self, Channel, Message, WebSocket};
use std::sync::Arc;
use surrealdb::Surreal;
use surrealdb::engine::any::Any;

struct AuthenticatedSession {
    user_id: String,
    token: String,
}

#[get("/?<token>")]
pub fn websocket_index(
    ws: WebSocket,
    hub: &State<Arc<ChatHub>>,
    db: &State<Surreal<Any>>,
    token: Option<String>,
) -> Channel<'static> {
    let hub = hub.inner().clone();
    let db = db.inner().clone();

    ws.channel(move |mut stream| {
        Box::pin(async move {
            let Some(mut session) = authenticate(&mut stream, token.as_deref()).await? else {
                return Ok(());
            };

            let mut rx = register_user(&hub, &session.user_id).await;

            let friends_online = hub.broadcast_presence(&db, &session.user_id, true).await;
            let auth_response = serde_json::json!({
                "status": "authenticated",
                "friends_online": friends_online,
            })
            .to_string();
            send_json(&mut stream, &auth_response).await?;

            run_message_loop(&mut stream, &mut rx, &hub, &db, &mut session).await?;

            disconnect_user(&hub, &session.user_id).await;
            hub.broadcast_presence(&db, &session.user_id, false).await;

            Ok(())
        })
    })
}

async fn authenticate(
    stream: &mut DuplexStream,
    query_token: Option<&str>,
) -> Result<Option<AuthenticatedSession>, rocket_ws::result::Error> {
    if let Some(token) = query_token {
        let session = decode_token(token).ok().map(|claims| AuthenticatedSession {
            user_id: claims.user_id,
            token: token.to_string(),
        });
        if let Some(s) = session {
            return Ok(Some(s));
        }
        send_json(stream, r#"{"error":"invalid token"}"#).await?;
        return Ok(None);
    }

    loop {
        match stream.next().await {
            Some(Ok(Message::Text(text))) => match serde_json::from_str::<AuthMessage>(&text) {
                Ok(auth_msg) => {
                    if let Ok(claims) = decode_token(&auth_msg.token) {
                        return Ok(Some(AuthenticatedSession {
                            user_id: claims.user_id,
                            token: auth_msg.token,
                        }));
                    }
                    send_json(stream, r#"{"error":"invalid token"}"#).await?;
                    return Ok(None);
                }
                Err(_) => {
                    send_json(stream, r#"{"error":"expected auth message"}"#).await?;
                }
            },
            Some(Ok(Message::Close(_))) | None => return Ok(None),
            _ => {}
        }
    }
}

async fn register_user(hub: &Arc<ChatHub>, user_id: &str) -> broadcast::Receiver<String> {
    let (tx, rx) = broadcast::channel(100);
    let mut conns = hub.connections.write().await;
    conns.insert(user_id.to_string(), tx);
    rx
}

async fn disconnect_user(hub: &Arc<ChatHub>, user_id: &str) {
    let mut conns = hub.connections.write().await;
    conns.remove(user_id);
}

async fn run_message_loop(
    stream: &mut DuplexStream,
    rx: &mut broadcast::Receiver<String>,
    hub: &Arc<ChatHub>,
    db: &Surreal<Any>,
    session: &mut AuthenticatedSession,
) -> Result<(), rocket_ws::result::Error> {
    let mut auth_check = interval(Duration::from_secs(300));

    loop {
        tokio::select! {
            Some(msg) = stream.next() => {
                match msg? {
                    Message::Text(text) => {
                        handle_incoming_message(stream, db, &text, &session.user_id, hub).await?;
                    }
                    Message::Close(_) => break,
                    _ => {}
                }
            }
            Ok(msg) = rx.recv() => {
                stream.send(Message::Text(msg)).await?;
            }
            _ = auth_check.tick() => {
                if !handle_token_refresh(stream, db, session).await? {
                    break;
                }
            }
        }
    }

    Ok(())
}

async fn handle_incoming_message(
    stream: &mut DuplexStream,
    db: &Surreal<Any>,
    text: &str,
    sender_id: &str,
    hub: &Arc<ChatHub>,
) -> Result<(), rocket_ws::result::Error> {
    match serde_json::from_str::<ChatMessage>(text) {
        Ok(mut chat_msg) => {
            chat_msg.from = Some(sender_id.to_string());
            ChatHub::send_to(hub, db, &mut chat_msg).await;
        }
        Err(_) => {
            send_json(stream, r#"{"error":"invalid message format, expected: {\"to\": \"string\", \"message_type\": \"string\", \"content\": \"string\"}"#).await?;
        }
    }
    Ok(())
}

async fn handle_token_refresh(
    stream: &mut DuplexStream,
    db: &Surreal<Any>,
    session: &mut AuthenticatedSession,
) -> Result<bool, rocket_ws::result::Error> {
    if decode_token(&session.token).is_ok() {
        return Ok(true);
    }

    send_json(stream, r#"{"action":"token_refresh_required"}"#).await?;

    let timeout = tokio::time::sleep(Duration::from_secs(30));
    tokio::pin!(timeout);

    loop {
        tokio::select! {
            Some(msg) = stream.next() => {
                if let Ok(Message::Text(text)) = msg {
                    if let Ok(refresh_msg) = serde_json::from_str::<RefreshMessage>(&text) {
                        if let Ok((_, login_success)) = refresh_token(refresh_msg.refresh_token, db).await {
                            session.token = login_success.token.clone();
                            let ok = serde_json::json!({
                                "status": "token_refreshed",
                                "token": login_success.token,
                                "refresh_token": login_success.refresh_token
                            }).to_string();
                            send_json(stream, &ok).await?;
                            return Ok(true);
                        }
                    }
                }
            }
            _ = &mut timeout => {
                send_json(stream, r#"{"error":"token_refresh_timeout"}"#).await?;
                stream.send(Message::Close(None)).await?;
                return Ok(false);
            }
        }
    }
}

async fn send_json(stream: &mut DuplexStream, json: &str) -> Result<(), rocket_ws::result::Error> {
    stream.send(Message::Text(json.into())).await
}

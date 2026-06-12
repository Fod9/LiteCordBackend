use crate::models::db::{Attachment, Channel, DMChannel, Message, MessageWithAuthor, SimpleUser};
use crate::permissions::{get_channel_permissions, missing_permission_error, not_member_error};
use rocket::http::Status;
use surrealdb::Surreal;
use surrealdb::engine::any::Any;
use surrealdb::sql::Thing;

pub async fn assert_channel_access(
    db: &Surreal<Any>,
    channel_id: &str,
    user_id: &str,
) -> Result<(), (Status, String)> {
    let channel_thing = surrealdb::sql::thing(channel_id)
        .map_err(|e| (Status::BadRequest, e.to_string()))?;

    match channel_thing.tb.as_str() {
        "DMChannel" => {
            let user_thing = surrealdb::sql::thing(user_id)
                .map_err(|e| (Status::BadRequest, e.to_string()))?;

            let dm: Option<DMChannel> = db
                .query("SELECT * FROM $channel_id")
                .bind(("channel_id", channel_thing))
                .await
                .map_err(|e| (Status::InternalServerError, e.to_string()))?
                .take(0)
                .map_err(|e| (Status::InternalServerError, e.to_string()))?;

            let dm = dm.ok_or((Status::NotFound, "Channel not found".to_string()))?;

            if !dm.recipients.iter().any(|r| r.to_raw() == user_thing.to_raw()) {
                return Err(not_member_error());
            }
            Ok(())
        }
        "channel" => {
            let channel: Option<Channel> = db
                .query("SELECT * FROM $channel_id")
                .bind(("channel_id", channel_thing))
                .await
                .map_err(|e| (Status::InternalServerError, e.to_string()))?
                .take(0)
                .map_err(|e| (Status::InternalServerError, e.to_string()))?;

            let channel = channel.ok_or((Status::NotFound, "Channel not found".to_string()))?;

            let perms = get_channel_permissions(db, &channel, user_id).await?;
            if !perms.has("view_channels") {
                return Err(missing_permission_error("view_channels"));
            }
            Ok(())
        }
        _ => Err((Status::BadRequest, "Invalid channel id".to_string())),
    }
}

async fn fetch_author(db: &Surreal<Any>, author_id: &Thing) -> Option<SimpleUser> {
    db.query("SELECT id, name, display_name, profile_picture FROM user WHERE id = $id")
        .bind(("id", author_id.clone()))
        .await
        .ok()?
        .take::<Vec<SimpleUser>>(0)
        .ok()?
        .into_iter()
        .next()
}

fn to_message_with_author(msg: Message, author: SimpleUser) -> MessageWithAuthor {
    MessageWithAuthor {
        id: msg.id,
        channel: msg.channel,
        author,
        content: msg.content,
        reply_to: msg.reply_to,
        attachments: msg.attachments,
        edited_at: msg.edited_at,
        created_at: msg.created_at,
    }
}

pub async fn save_message_with_author(
    db: &Surreal<Any>,
    channel_id: &str,
    author_id: &str,
    content: &str,
    attachments: Vec<Attachment>,
) -> Result<MessageWithAuthor, String> {
    let msg = save_message(db, channel_id, author_id, content, attachments).await?;
    let author = fetch_author(db, &msg.author)
        .await
        .ok_or_else(|| format!("Author not found: {}", msg.author.to_raw()))?;
    Ok(to_message_with_author(msg, author))
}

pub async fn save_message(
    db: &Surreal<Any>,
    channel_id: &str,
    author_id: &str,
    content: &str,
    attachments: Vec<Attachment>,
) -> Result<Message, String> {
    let channel_thing = surrealdb::sql::thing(channel_id).map_err(|e| e.to_string())?;
    let author_thing = surrealdb::sql::thing(author_id).map_err(|e| e.to_string())?;

    let mut result = db
        .query("CREATE message SET channel = $channel, author = $author, content = $content, attachments = $attachments, created_at = time::now()")
        .bind(("channel", channel_thing))
        .bind(("author", author_thing))
        .bind(("content", content.to_string()))
        .bind(("attachments", attachments))
        .await
        .map_err(|e| e.to_string())?;

    result
        .take::<Vec<Message>>(0)
        .map_err(|e| e.to_string())?
        .pop()
        .ok_or_else(|| "Failed to create message".to_string())
}

pub async fn get_channel_messages(
    db: &Surreal<Any>,
    channel_id: &str,
    limit: u32,
    before: Option<String>,
) -> Result<Vec<MessageWithAuthor>, String> {
    let channel_thing = surrealdb::sql::thing(channel_id).map_err(|e| e.to_string())?;
    let limit = limit.min(100) as i64;

    let mut messages: Vec<Message> = match before {
        Some(before_id) => {
            let before_thing = surrealdb::sql::thing(&before_id).map_err(|e| e.to_string())?;
            db.query(
                "SELECT * FROM message
                 WHERE channel = $channel
                   AND created_at < (SELECT VALUE created_at FROM ONLY message WHERE id = $before LIMIT 1)
                 ORDER BY created_at DESC
                 LIMIT $limit",
            )
            .bind(("channel", channel_thing))
            .bind(("before", before_thing))
            .bind(("limit", limit))
            .await
            .map_err(|e| e.to_string())?
            .take::<Vec<Message>>(0)
            .map_err(|e| e.to_string())?
        }
        None => {
            db.query(
                "SELECT * FROM message
                 WHERE channel = $channel
                 ORDER BY created_at DESC
                 LIMIT $limit",
            )
            .bind(("channel", channel_thing))
            .bind(("limit", limit))
            .await
            .map_err(|e| e.to_string())?
            .take::<Vec<Message>>(0)
            .map_err(|e| e.to_string())?
        }
    };

    messages.reverse();

    let mut enriched = Vec::with_capacity(messages.len());
    for msg in messages {
        let author = fetch_author(db, &msg.author)
            .await
            .ok_or_else(|| format!("Author not found: {}", msg.author.to_raw()))?;
        enriched.push(to_message_with_author(msg, author));
    }
    Ok(enriched)
}

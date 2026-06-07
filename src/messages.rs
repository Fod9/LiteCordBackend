use crate::models::db::Message;
use surrealdb::Surreal;
use surrealdb::engine::any::Any;

pub async fn save_message(
    db: &Surreal<Any>,
    channel_id: &str,
    author_id: &str,
    content: &str,
) -> Result<Message, String> {
    let channel_thing = surrealdb::sql::thing(channel_id).map_err(|e| e.to_string())?;
    let author_thing = surrealdb::sql::thing(author_id).map_err(|e| e.to_string())?;

    let mut result = db
        .query("CREATE message SET channel = $channel, author = $author, content = $content, attachments = []")
        .bind(("channel", channel_thing))
        .bind(("author", author_thing))
        .bind(("content", content.to_string()))
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
) -> Result<Vec<Message>, String> {
    let channel_thing = surrealdb::sql::thing(channel_id).map_err(|e| e.to_string())?;
    let limit = limit.min(100) as i64;

    let mut messages = match before {
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
    Ok(messages)
}

use rocket::http::Status;
use surrealdb::Surreal;
use surrealdb::engine::any::Any;
use surrealdb::sql::Thing;
use crate::friends::are_accepted_friends;
use crate::models::db::{DMChannel, DMChannelWithParticipants, Friendship, SimpleUser};

pub async fn list_channels_for_user(user_id: &str, db: &Surreal<Any>) -> Result<(Vec<DMChannelWithParticipants>, Vec<Friendship>), String> {
    let user_thing = surrealdb::sql::thing(user_id).map_err(|e| e.to_string())?;

    let query = "SELECT * FROM DMChannel WHERE $user_id IN recipients FETCH recipients";
    let query_friends = "SELECT * FROM friendship WHERE (`in` = $user_id OR out = $user_id) AND status = 'accepted'";

    let channels = db
        .query(query)
        .bind(("user_id", user_thing.clone()))
        .await
        .map_err(|e| e.to_string())?
        .take::<Vec<DMChannelWithParticipants>>(0)
        .map_err(|e| e.to_string())?;

    let friendships = db
        .query(query_friends)
        .bind(("user_id", user_thing))
        .await
        .map_err(|e| e.to_string())?
        .take::<Vec<Friendship>>(0)
        .map_err(|e| e.to_string())?;

    Ok((channels, friendships))
}

async fn fetch_dm_channel_with_participants(
    channel_id: &str,
    db: &Surreal<Any>,
) -> Result<DMChannelWithParticipants, (Status, String)> {
    let thing = surrealdb::sql::thing(channel_id)
        .map_err(|e| (Status::InternalServerError, e.to_string()))?;

    let channels: Vec<DMChannelWithParticipants> = db
        .query("SELECT * FROM $id FETCH recipients")
        .bind(("id", thing))
        .await
        .map_err(|e| (Status::InternalServerError, e.to_string()))?
        .take(0)
        .map_err(|e| (Status::InternalServerError, e.to_string()))?;

    channels.into_iter().next()
        .ok_or_else(|| (Status::InternalServerError, "channel not found after create".to_string()))
}

pub async fn create_dm_channel(
    owner_id: &str,
    recipient_ids: Vec<String>,
    db: &Surreal<Any>,
) -> Result<DMChannelWithParticipants, (Status, String)> {
    let owner_thing = surrealdb::sql::thing(owner_id)
        .map_err(|e| (Status::BadRequest, e.to_string()))?;

    let mut all_things: Vec<Thing> = vec![owner_thing.clone()];
    for id in &recipient_ids {
        let t = surrealdb::sql::thing(id)
            .map_err(|e| (Status::BadRequest, format!("invalid recipient id {id}: {e}")))?;
        if !all_things.iter().any(|x| x == &t) {
            all_things.push(t);
        }
    }

    let found: Vec<SimpleUser> = db
        .query("SELECT id, name, display_name, profile_picture FROM user WHERE id IN $ids")
        .bind(("ids", all_things.clone()))
        .await
        .map_err(|e| (Status::InternalServerError, e.to_string()))?
        .take(0)
        .map_err(|e| (Status::InternalServerError, e.to_string()))?;

    if found.len() < all_things.len() {
        return Err((Status::NotFound, "one or more recipients not found".to_string()));
    }

    for t in &all_things {
        if t == &owner_thing {
            continue;
        }
        if !are_accepted_friends(db, &owner_thing.to_raw(), &t.to_raw()).await {
            return Err((Status::Forbidden, "you must be friends with all recipients".to_string()));
        }
    }

    let mut sorted_ids: Vec<String> = all_things.iter().map(|t| t.to_raw()).collect();
    sorted_ids.sort();
    let recipients_key = sorted_ids.join(",");

    let existing: Vec<DMChannel> = db
        .query("SELECT * FROM DMChannel WHERE recipients_key = $rk")
        .bind(("rk", recipients_key.clone()))
        .await
        .map_err(|e| (Status::InternalServerError, e.to_string()))?
        .take(0)
        .map_err(|e| (Status::InternalServerError, e.to_string()))?;

    if let Some(channel) = existing.into_iter().next() {
        let id = channel.id.ok_or_else(|| (Status::InternalServerError, "channel has no id".to_string()))?;
        return fetch_dm_channel_with_participants(&id.to_raw(), db).await;
    }

    let created: Vec<DMChannel> = db
        .query("CREATE DMChannel SET recipients = $recipients, owner = $owner, recipients_key = $rk, created_at = time::now()")
        .bind(("recipients", all_things))
        .bind(("owner", owner_thing))
        .bind(("rk", recipients_key))
        .await
        .map_err(|e| (Status::InternalServerError, e.to_string()))?
        .take(0)
        .map_err(|e| (Status::InternalServerError, e.to_string()))?;

    let channel_id = created.into_iter().next()
        .and_then(|c| c.id)
        .ok_or_else(|| (Status::InternalServerError, "failed to create dm channel".to_string()))?;

    fetch_dm_channel_with_participants(&channel_id.to_raw(), db).await
}

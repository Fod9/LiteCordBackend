use crate::models::db::{Channel, ChannelType, Guild, MemberOf};
use rocket::http::Status;
use surrealdb::Surreal;
use surrealdb::engine::any::Any;

async fn assert_member(
    db: &Surreal<Any>,
    guild_id: &surrealdb::sql::Thing,
    user_id: &surrealdb::sql::Thing,
) -> Result<(), (Status, String)> {
    let membership: Vec<MemberOf> = db
        .query("SELECT * FROM member_of WHERE `in` = $user_id AND out = $guild_id")
        .bind(("user_id", user_id.clone()))
        .bind(("guild_id", guild_id.clone()))
        .await
        .map_err(|e| (Status::InternalServerError, e.to_string()))?
        .take(0)
        .map_err(|e| (Status::InternalServerError, e.to_string()))?;

    if membership.is_empty() {
        return Err((Status::Forbidden, "You are not a member of this guild".to_string()));
    }
    Ok(())
}

async fn get_guild_owner(
    db: &Surreal<Any>,
    guild_id: &surrealdb::sql::Thing,
) -> Result<surrealdb::sql::Thing, (Status, String)> {
    let guild: Option<Guild> = db
        .query("SELECT * FROM $guild_id")
        .bind(("guild_id", guild_id.clone()))
        .await
        .map_err(|e| (Status::InternalServerError, e.to_string()))?
        .take(0)
        .map_err(|e| (Status::InternalServerError, e.to_string()))?;

    guild
        .ok_or((Status::NotFound, "Guild not found".to_string()))
        .map(|g| g.owner)
}

pub async fn create_channel(
    db: &Surreal<Any>,
    guild_id: &str,
    user_id: &str,
    name: String,
    channel_type: ChannelType,
    category: Option<String>,
) -> Result<Channel, (Status, String)> {
    let guild_thing = surrealdb::sql::thing(guild_id)
        .map_err(|e| (Status::BadRequest, e.to_string()))?;
    let user_thing = surrealdb::sql::thing(user_id)
        .map_err(|e| (Status::BadRequest, e.to_string()))?;

    assert_member(db, &guild_thing, &user_thing).await?;

    let type_str = match channel_type {
        ChannelType::Text => "Text",
        ChannelType::Voice => "Voice",
    };

    let channel: Option<Channel> = db
        .query(
            "CREATE channel SET
             guild = $guild,
             name = $name,
             channel_type = $channel_type,
             category = $category,
             created_at = time::now()",
        )
        .bind(("guild", guild_thing))
        .bind(("name", name))
        .bind(("channel_type", type_str))
        .bind(("category", category))
        .await
        .map_err(|e| (Status::InternalServerError, e.to_string()))?
        .take(0)
        .map_err(|e| (Status::InternalServerError, e.to_string()))?;

    channel.ok_or((Status::InternalServerError, "Failed to create channel".to_string()))
}

pub async fn list_guild_channels(
    db: &Surreal<Any>,
    guild_id: &str,
    user_id: &str,
) -> Result<Vec<Channel>, (Status, String)> {
    let guild_thing = surrealdb::sql::thing(guild_id)
        .map_err(|e| (Status::BadRequest, e.to_string()))?;
    let user_thing = surrealdb::sql::thing(user_id)
        .map_err(|e| (Status::BadRequest, e.to_string()))?;

    assert_member(db, &guild_thing, &user_thing).await?;

    db.query("SELECT * FROM channel WHERE guild = $guild ORDER BY created_at ASC")
        .bind(("guild", guild_thing))
        .await
        .map_err(|e| (Status::InternalServerError, e.to_string()))?
        .take(0)
        .map_err(|e| (Status::InternalServerError, e.to_string()))
}

pub async fn delete_channel(
    db: &Surreal<Any>,
    guild_id: &str,
    channel_id: &str,
    user_id: &str,
) -> Result<(), (Status, String)> {
    let guild_thing = surrealdb::sql::thing(guild_id)
        .map_err(|e| (Status::BadRequest, e.to_string()))?;
    let channel_thing = surrealdb::sql::thing(channel_id)
        .map_err(|e| (Status::BadRequest, e.to_string()))?;
    let user_thing = surrealdb::sql::thing(user_id)
        .map_err(|e| (Status::BadRequest, e.to_string()))?;

    let owner = get_guild_owner(db, &guild_thing).await?;

    if owner.to_raw() != user_thing.to_raw() {
        return Err((Status::Forbidden, "Only the guild owner can delete channels".to_string()));
    }

    db.query("DELETE message WHERE channel = $channel_id")
        .bind(("channel_id", channel_thing.clone()))
        .await
        .map_err(|e| (Status::InternalServerError, e.to_string()))?;

    db.query("DELETE $channel_id")
        .bind(("channel_id", channel_thing))
        .await
        .map_err(|e| (Status::InternalServerError, e.to_string()))?;

    Ok(())
}

pub async fn get_guild_member_ids(
    db: &Surreal<Any>,
    channel_id: &surrealdb::sql::Thing,
) -> Result<Vec<surrealdb::sql::Thing>, String> {
    let channel: Option<Channel> = db
        .query("SELECT * FROM $channel_id")
        .bind(("channel_id", channel_id.clone()))
        .await
        .map_err(|e| e.to_string())?
        .take(0)
        .map_err(|e| e.to_string())?;

    let guild_thing = channel
        .ok_or_else(|| "Channel not found".to_string())?
        .guild;

    let memberships: Vec<MemberOf> = db
        .query("SELECT * FROM member_of WHERE out = $guild_id")
        .bind(("guild_id", guild_thing))
        .await
        .map_err(|e| e.to_string())?
        .take(0)
        .map_err(|e| e.to_string())?;

    Ok(memberships.into_iter().map(|m| m.user).collect())
}

use crate::models::db::{Channel, ChannelType, MemberOf, PermissionOverwrite};
use crate::permissions::{
    get_member_permissions_with_roles, require_permission, resolve_channel_overwrites,
    unknown_permissions, unknown_permissions_error,
};
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

    require_permission(db, guild_id, user_id, "manage_channels").await?;

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

    let (base, role_ids) = get_member_permissions_with_roles(db, guild_id, user_id).await?;

    let channels: Vec<Channel> = db
        .query("SELECT * FROM channel WHERE guild = $guild ORDER BY created_at ASC")
        .bind(("guild", guild_thing))
        .await
        .map_err(|e| (Status::InternalServerError, e.to_string()))?
        .take(0)
        .map_err(|e| (Status::InternalServerError, e.to_string()))?;

    Ok(channels
        .into_iter()
        .filter(|channel| {
            let perms = resolve_channel_overwrites(
                base.clone(),
                &role_ids,
                user_id,
                &channel.permission_overwrites,
            );
            perms.has("view_channels")
        })
        .collect())
}

pub async fn update_channel_permissions(
    db: &Surreal<Any>,
    guild_id: &str,
    channel_id: &str,
    user_id: &str,
    overwrites: Vec<PermissionOverwrite>,
) -> Result<Channel, (Status, String)> {
    let guild_thing = surrealdb::sql::thing(guild_id)
        .map_err(|e| (Status::BadRequest, e.to_string()))?;
    let channel_thing = surrealdb::sql::thing(channel_id)
        .map_err(|e| (Status::BadRequest, e.to_string()))?;

    require_permission(db, guild_id, user_id, "manage_channels").await?;

    let mut unknown: Vec<String> = vec![];
    for ow in &overwrites {
        let target = surrealdb::sql::thing(&ow.target)
            .map_err(|_| (Status::BadRequest, format!(r#"{{"error":"invalid_target","target":"{}"}}"#, ow.target)))?;
        if target.tb != "role" && target.tb != "user" {
            return Err((
                Status::BadRequest,
                format!(r#"{{"error":"invalid_target","target":"{}"}}"#, ow.target),
            ));
        }
        unknown.extend(unknown_permissions(&ow.allow));
        unknown.extend(unknown_permissions(&ow.deny));
    }
    if !unknown.is_empty() {
        return Err(unknown_permissions_error(&unknown));
    }

    let channel: Option<Channel> = db
        .query("SELECT * FROM $channel_id WHERE guild = $guild_id")
        .bind(("channel_id", channel_thing.clone()))
        .bind(("guild_id", guild_thing))
        .await
        .map_err(|e| (Status::InternalServerError, e.to_string()))?
        .take(0)
        .map_err(|e| (Status::InternalServerError, e.to_string()))?;

    channel.ok_or((Status::NotFound, "Channel not found or does not belong to this guild".to_string()))?;

    let updated: Option<Channel> = db
        .query("UPDATE $channel_id SET permission_overwrites = $overwrites")
        .bind(("channel_id", channel_thing))
        .bind(("overwrites", overwrites))
        .await
        .map_err(|e| (Status::InternalServerError, e.to_string()))?
        .take(0)
        .map_err(|e| (Status::InternalServerError, e.to_string()))?;

    updated.ok_or((Status::InternalServerError, "Failed to update channel permissions".to_string()))
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

    require_permission(db, guild_id, user_id, "manage_channels").await?;

    let channel: Option<Channel> = db
        .query("SELECT * FROM $channel_id WHERE guild = $guild_id")
        .bind(("channel_id", channel_thing.clone()))
        .bind(("guild_id", guild_thing))
        .await
        .map_err(|e| (Status::InternalServerError, e.to_string()))?
        .take(0)
        .map_err(|e| (Status::InternalServerError, e.to_string()))?;

    channel.ok_or((Status::NotFound, "Channel not found or does not belong to this guild".to_string()))?;

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

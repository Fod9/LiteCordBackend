use crate::chat::hub::ChatHub;
use crate::chat::types::ServerMessage;
use crate::guild_channels::{
    create_channel, delete_channel, list_guild_channels, update_channel_permissions,
};
use crate::models::db::{ChannelType, PermissionOverwrite};
use crate::models::user::AuthenticatedUser;
use rocket::http::Status;
use rocket::serde::json::Json;
use rocket::{State, delete, get, post, put};
use serde::Deserialize;
use std::sync::Arc;
use surrealdb::Surreal;
use surrealdb::engine::any::Any;

#[derive(Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct CreateChannelRequest {
    pub name: String,
    pub channel_type: String,
    pub category: Option<String>,
}

#[derive(Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct UpdateChannelPermissionsRequest {
    pub permission_overwrites: Vec<PermissionOverwrite>,
}

#[post("/<guild_id>/channels", rank = 2, format = "json", data = "<body>")]
pub async fn create_channel_route(
    token: AuthenticatedUser,
    guild_id: String,
    body: Json<CreateChannelRequest>,
    db: &State<Surreal<Any>>,
    hub: &State<Arc<ChatHub>>,
) -> Result<(Status, String), (Status, String)> {
    let body = body.into_inner();

    let channel_type = match body.channel_type.as_str() {
        "Text" => ChannelType::Text,
        "Voice" => ChannelType::Voice,
        _ => return Err((Status::BadRequest, "channel_type must be 'Text' or 'Voice'".to_string())),
    };

    let channel = create_channel(db, &guild_id, &token.user_id, body.name, channel_type, body.category).await?;
    let json = serde_json::to_string(&channel)
        .map_err(|e| (Status::InternalServerError, e.to_string()))?;

    let event = serde_json::to_string(&ServerMessage {
        message_type: "channel_created".to_string(),
        content: json.clone(),
    })
    .unwrap_or_default();
    hub.broadcast_to_guild_members(db, &guild_id, &event).await;

    Ok((Status::Created, json))
}

#[get("/<guild_id>/channels")]
pub async fn list_channels_route(
    token: AuthenticatedUser,
    guild_id: String,
    db: &State<Surreal<Any>>,
) -> Result<(Status, String), (Status, String)> {
    match list_guild_channels(db, &guild_id, &token.user_id).await {
        Ok(channels) => {
            let json = serde_json::to_string(&channels)
                .map_err(|e| (Status::InternalServerError, e.to_string()))?;
            Ok((Status::Ok, json))
        }
        Err(e) => Err(e),
    }
}

#[delete("/<guild_id>/channels/<channel_id>")]
pub async fn delete_channel_route(
    token: AuthenticatedUser,
    guild_id: String,
    channel_id: String,
    db: &State<Surreal<Any>>,
    hub: &State<Arc<ChatHub>>,
) -> Result<Status, (Status, String)> {
    delete_channel(db, &guild_id, &channel_id, &token.user_id).await?;

    hub.clear_channel_voice_states(db, &guild_id, &channel_id).await;

    let payload = serde_json::json!({
        "guild_id": guild_id,
        "channel_id": channel_id
    })
    .to_string();
    let event = serde_json::to_string(&ServerMessage {
        message_type: "channel_deleted".to_string(),
        content: payload,
    })
    .unwrap_or_default();
    hub.broadcast_to_guild_members(db, &guild_id, &event).await;

    Ok(Status::NoContent)
}

#[put("/<guild_id>/channels/<channel_id>/permissions", format = "json", data = "<body>")]
pub async fn update_channel_permissions_route(
    token: AuthenticatedUser,
    guild_id: String,
    channel_id: String,
    body: Json<UpdateChannelPermissionsRequest>,
    db: &State<Surreal<Any>>,
    hub: &State<Arc<ChatHub>>,
) -> Result<(Status, String), (Status, String)> {
    let channel = update_channel_permissions(
        db,
        &guild_id,
        &channel_id,
        &token.user_id,
        body.into_inner().permission_overwrites,
    )
    .await?;

    let json = serde_json::to_string(&channel)
        .map_err(|e| (Status::InternalServerError, e.to_string()))?;

    let event = serde_json::to_string(&ServerMessage {
        message_type: "channel_permissions_updated".to_string(),
        content: json.clone(),
    })
    .unwrap_or_default();
    hub.broadcast_to_guild_members(db, &guild_id, &event).await;

    Ok((Status::Ok, json))
}

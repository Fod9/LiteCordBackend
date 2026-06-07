use crate::guild_channels::{create_channel, delete_channel, list_guild_channels};
use crate::models::db::ChannelType;
use crate::models::user::AuthenticatedUser;
use rocket::http::Status;
use rocket::serde::json::Json;
use rocket::{State, delete, get, post};
use serde::Deserialize;
use surrealdb::Surreal;
use surrealdb::engine::any::Any;

#[derive(Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct CreateChannelRequest {
    pub name: String,
    pub channel_type: String,
    pub category: Option<String>,
}

#[post("/<guild_id>/channels", rank = 2, format = "json", data = "<body>")]
pub async fn create_channel_route(
    token: AuthenticatedUser,
    guild_id: String,
    body: Json<CreateChannelRequest>,
    db: &State<Surreal<Any>>,
) -> Result<(Status, String), (Status, String)> {
    let body = body.into_inner();

    let channel_type = match body.channel_type.as_str() {
        "Text" => ChannelType::Text,
        "Voice" => ChannelType::Voice,
        _ => return Err((Status::BadRequest, "channel_type must be 'Text' or 'Voice'".to_string())),
    };

    match create_channel(db, &guild_id, &token.user_id, body.name, channel_type, body.category).await {
        Ok(channel) => {
            let json = serde_json::to_string(&channel)
                .map_err(|e| (Status::InternalServerError, e.to_string()))?;
            Ok((Status::Created, json))
        }
        Err(e) => Err(e),
    }
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
) -> Result<Status, (Status, String)> {
    delete_channel(db, &guild_id, &channel_id, &token.user_id)
        .await
        .map(|_| Status::NoContent)
}

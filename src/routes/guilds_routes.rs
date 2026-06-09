use crate::chat::hub::ChatHub;
use crate::chat::types::ServerMessage;
use crate::guilds::{
    create_guild, create_invite, delete_guild, join_guild, kick_member, leave_guild,
    list_guild_invites, list_guild_members, list_user_guilds, revoke_invite, update_guild,
};
use crate::models::db::SimpleUser;
use crate::models::user::AuthenticatedUser;
use rocket::http::Status;
use rocket::serde::json::Json;
use rocket::{State, delete, get, patch, post};
use serde::Deserialize;
use std::sync::Arc;
use surrealdb::Surreal;
use surrealdb::engine::any::Any;

#[derive(Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct CreateGuildRequest {
    pub name: String,
    pub icon: String,
}

#[derive(Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct UpdateGuildRequest {
    pub name: Option<String>,
    pub icon: Option<String>,
}

#[post("/", format = "json", data = "<body>")]
pub async fn create_guild_route(
    token: AuthenticatedUser,
    body: Json<CreateGuildRequest>,
    db: &State<Surreal<Any>>,
) -> Result<(Status, String), (Status, String)> {
    let body = body.into_inner();
    match create_guild(db, &token.user_id, body.name, body.icon).await {
        Ok(guild) => {
            let json = serde_json::to_string(&guild)
                .map_err(|e| (Status::InternalServerError, e.to_string()))?;
            Ok((Status::Created, json))
        }
        Err(e) => Err((Status::InternalServerError, e)),
    }
}

#[get("/")]
pub async fn list_guilds_route(
    token: AuthenticatedUser,
    db: &State<Surreal<Any>>,
) -> Result<(Status, String), (Status, String)> {
    match list_user_guilds(db, &token.user_id).await {
        Ok(guilds) => {
            let json = serde_json::to_string(&guilds)
                .map_err(|e| (Status::InternalServerError, e.to_string()))?;
            Ok((Status::Ok, json))
        }
        Err(e) => Err((Status::InternalServerError, e)),
    }
}

#[delete("/<guild_id>")]
pub async fn delete_guild_route(
    token: AuthenticatedUser,
    guild_id: String,
    db: &State<Surreal<Any>>,
    hub: &State<Arc<ChatHub>>,
) -> Result<Status, (Status, String)> {
    let member_ids = delete_guild(db, &guild_id, &token.user_id).await?;

    let event = serde_json::to_string(&ServerMessage {
        message_type: "guild_deleted".to_string(),
        content: guild_id,
    })
    .unwrap_or_default();

    for member_id in &member_ids {
        hub.forward_to_client(member_id, &event).await;
    }

    Ok(Status::NoContent)
}

#[post("/<guild_id>/leave", rank = 2)]
pub async fn leave_guild_route(
    token: AuthenticatedUser,
    guild_id: String,
    db: &State<Surreal<Any>>,
    hub: &State<Arc<ChatHub>>,
) -> Result<Status, (Status, String)> {
    leave_guild(db, &guild_id, &token.user_id).await?;

    let payload = serde_json::json!({
        "guild_id": guild_id,
        "user_id": token.user_id
    })
    .to_string();
    let event = serde_json::to_string(&ServerMessage {
        message_type: "guild_member_left".to_string(),
        content: payload,
    })
    .unwrap_or_default();
    hub.broadcast_to_guild_members(db, &guild_id, &event).await;

    Ok(Status::NoContent)
}

#[post("/<guild_id>/invites", rank = 2)]
pub async fn create_invite_route(
    token: AuthenticatedUser,
    guild_id: String,
    db: &State<Surreal<Any>>,
) -> Result<(Status, String), (Status, String)> {
    match create_invite(db, &guild_id, &token.user_id).await {
        Ok(invite) => {
            let json = serde_json::to_string(&invite)
                .map_err(|e| (Status::InternalServerError, e.to_string()))?;
            Ok((Status::Created, json))
        }
        Err(e) => Err(e),
    }
}

#[post("/join/<code>", rank = 1)]
pub async fn join_guild_route(
    token: AuthenticatedUser,
    code: String,
    db: &State<Surreal<Any>>,
    hub: &State<Arc<ChatHub>>,
) -> Result<(Status, String), (Status, String)> {
    let guild = join_guild(db, &code, &token.user_id).await?;

    if let Some(guild_id) = guild.id.as_ref() {
        let guild_id_raw = guild_id.to_raw();
        let user: Option<SimpleUser> =
            match surrealdb::sql::thing(&token.user_id) {
                Ok(user_thing) => db
                    .query("SELECT id, name, display_name, profile_picture FROM user WHERE id = $id")
                    .bind(("id", user_thing))
                    .await
                    .ok()
                    .and_then(|mut r| r.take::<Vec<SimpleUser>>(0).ok())
                    .and_then(|mut v| v.pop()),
                Err(_) => None,
            };

        if let Some(user) = user {
            let payload = serde_json::json!({
                "guild_id": guild_id_raw,
                "user": user
            })
            .to_string();
            let event = serde_json::to_string(&ServerMessage {
                message_type: "guild_member_joined".to_string(),
                content: payload,
            })
            .unwrap_or_default();
            hub.broadcast_to_guild_members(db, &guild_id_raw, &event).await;
        }
    }

    let json = serde_json::to_string(&guild)
        .map_err(|e| (Status::InternalServerError, e.to_string()))?;
    Ok((Status::Ok, json))
}

#[get("/<guild_id>/members")]
pub async fn list_guild_members_route(
    token: AuthenticatedUser,
    guild_id: String,
    db: &State<Surreal<Any>>,
) -> Result<(Status, String), (Status, String)> {
    match list_guild_members(db, &guild_id, &token.user_id).await {
        Ok(members) => {
            let json = serde_json::to_string(&members)
                .map_err(|e| (Status::InternalServerError, e.to_string()))?;
            Ok((Status::Ok, json))
        }
        Err(e) => Err(e),
    }
}

#[post("/<guild_id>/members/<user_id>/kick")]
pub async fn kick_member_route(
    token: AuthenticatedUser,
    guild_id: String,
    user_id: String,
    db: &State<Surreal<Any>>,
    hub: &State<Arc<ChatHub>>,
) -> Result<Status, (Status, String)> {
    kick_member(db, &guild_id, &user_id, &token.user_id).await?;

    let payload = serde_json::json!({
        "guild_id": guild_id,
        "user_id": user_id
    })
    .to_string();
    let event = serde_json::to_string(&ServerMessage {
        message_type: "guild_member_left".to_string(),
        content: payload,
    })
    .unwrap_or_default();
    hub.broadcast_to_guild_members(db, &guild_id, &event).await;

    Ok(Status::NoContent)
}

#[patch("/<guild_id>", format = "json", data = "<body>")]
pub async fn update_guild_route(
    token: AuthenticatedUser,
    guild_id: String,
    body: Json<UpdateGuildRequest>,
    db: &State<Surreal<Any>>,
) -> Result<(Status, String), (Status, String)> {
    let body = body.into_inner();
    match update_guild(db, &guild_id, &token.user_id, body.name, body.icon).await {
        Ok(guild) => {
            let json = serde_json::to_string(&guild)
                .map_err(|e| (Status::InternalServerError, e.to_string()))?;
            Ok((Status::Ok, json))
        }
        Err(e) => Err(e),
    }
}

#[get("/<guild_id>/invites")]
pub async fn list_guild_invites_route(
    token: AuthenticatedUser,
    guild_id: String,
    db: &State<Surreal<Any>>,
) -> Result<(Status, String), (Status, String)> {
    match list_guild_invites(db, &guild_id, &token.user_id).await {
        Ok(invites) => {
            let json = serde_json::to_string(&invites)
                .map_err(|e| (Status::InternalServerError, e.to_string()))?;
            Ok((Status::Ok, json))
        }
        Err(e) => Err(e),
    }
}

#[delete("/<guild_id>/invites/<invite_id>")]
pub async fn revoke_invite_route(
    token: AuthenticatedUser,
    guild_id: String,
    invite_id: String,
    db: &State<Surreal<Any>>,
) -> Result<Status, (Status, String)> {
    revoke_invite(db, &guild_id, &invite_id, &token.user_id)
        .await
        .map(|_| Status::NoContent)
}

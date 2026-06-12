use crate::chat::hub::ChatHub;
use crate::chat::types::ServerMessage;
use crate::models::user::AuthenticatedUser;
use crate::roles::{assign_role, create_role, delete_role, list_roles, remove_role, update_role};
use rocket::http::Status;
use rocket::serde::json::Json;
use rocket::{State, delete, get, patch, post};
use serde::Deserialize;
use std::sync::Arc;
use surrealdb::Surreal;
use surrealdb::engine::any::Any;

#[derive(Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct CreateRoleRequest {
    pub name: String,
    pub color: String,
    pub position: i32,
    pub permissions: Vec<String>,
}

#[derive(Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct UpdateRoleRequest {
    pub name: Option<String>,
    pub color: Option<String>,
    pub position: Option<i32>,
    pub permissions: Option<Vec<String>>,
}

async fn broadcast_role_event(
    hub: &ChatHub,
    db: &Surreal<Any>,
    guild_id: &str,
    message_type: &str,
    content: String,
) {
    let event = serde_json::to_string(&ServerMessage {
        message_type: message_type.to_string(),
        content,
    })
    .unwrap_or_default();
    hub.broadcast_to_guild_members(db, guild_id, &event).await;
}

#[post("/<guild_id>/roles", rank = 2, format = "json", data = "<body>")]
pub async fn create_role_route(
    token: AuthenticatedUser,
    guild_id: String,
    body: Json<CreateRoleRequest>,
    db: &State<Surreal<Any>>,
    hub: &State<Arc<ChatHub>>,
) -> Result<(Status, String), (Status, String)> {
    let body = body.into_inner();
    match create_role(db, &guild_id, &token.user_id, body.name, body.color, body.position, body.permissions).await {
        Ok(role) => {
            let json = serde_json::to_string(&role)
                .map_err(|e| (Status::InternalServerError, e.to_string()))?;
            broadcast_role_event(hub, db, &guild_id, "role_created", json.clone()).await;
            Ok((Status::Created, json))
        }
        Err(e) => Err(e),
    }
}

#[patch("/<guild_id>/roles/<role_id>", format = "json", data = "<body>")]
pub async fn update_role_route(
    token: AuthenticatedUser,
    guild_id: String,
    role_id: String,
    body: Json<UpdateRoleRequest>,
    db: &State<Surreal<Any>>,
    hub: &State<Arc<ChatHub>>,
) -> Result<(Status, String), (Status, String)> {
    let body = body.into_inner();
    match update_role(db, &guild_id, &role_id, &token.user_id, body.name, body.color, body.position, body.permissions).await {
        Ok(role) => {
            let json = serde_json::to_string(&role)
                .map_err(|e| (Status::InternalServerError, e.to_string()))?;
            broadcast_role_event(hub, db, &guild_id, "role_modified", json.clone()).await;
            Ok((Status::Ok, json))
        }
        Err(e) => Err(e),
    }
}

#[get("/<guild_id>/roles")]
pub async fn list_roles_route(
    token: AuthenticatedUser,
    guild_id: String,
    db: &State<Surreal<Any>>,
) -> Result<(Status, String), (Status, String)> {
    match list_roles(db, &guild_id, &token.user_id).await {
        Ok(roles) => {
            let json = serde_json::to_string(&roles)
                .map_err(|e| (Status::InternalServerError, e.to_string()))?;
            Ok((Status::Ok, json))
        }
        Err(e) => Err(e),
    }
}

#[delete("/<guild_id>/roles/<role_id>")]
pub async fn delete_role_route(
    token: AuthenticatedUser,
    guild_id: String,
    role_id: String,
    db: &State<Surreal<Any>>,
    hub: &State<Arc<ChatHub>>,
) -> Result<Status, (Status, String)> {
    delete_role(db, &guild_id, &role_id, &token.user_id).await?;

    let payload = serde_json::json!({
        "guild_id": guild_id,
        "role_id": role_id
    })
    .to_string();
    broadcast_role_event(hub, db, &guild_id, "role_deleted", payload).await;

    Ok(Status::NoContent)
}

#[post("/<guild_id>/members/<target_user_id>/roles/<role_id>")]
pub async fn assign_role_route(
    token: AuthenticatedUser,
    guild_id: String,
    target_user_id: String,
    role_id: String,
    db: &State<Surreal<Any>>,
    hub: &State<Arc<ChatHub>>,
) -> Result<Status, (Status, String)> {
    assign_role(db, &guild_id, &role_id, &target_user_id, &token.user_id).await?;

    let payload = serde_json::json!({
        "guild_id": guild_id,
        "user_id": target_user_id,
        "role_id": role_id,
        "action": "assigned"
    })
    .to_string();
    let event = serde_json::to_string(&ServerMessage {
        message_type: "role_updated".to_string(),
        content: payload,
    })
    .unwrap_or_default();
    hub.broadcast_to_guild_members(db, &guild_id, &event).await;

    Ok(Status::NoContent)
}

#[delete("/<guild_id>/members/<target_user_id>/roles/<role_id>")]
pub async fn remove_role_route(
    token: AuthenticatedUser,
    guild_id: String,
    target_user_id: String,
    role_id: String,
    db: &State<Surreal<Any>>,
    hub: &State<Arc<ChatHub>>,
) -> Result<Status, (Status, String)> {
    remove_role(db, &guild_id, &role_id, &target_user_id, &token.user_id).await?;

    let payload = serde_json::json!({
        "guild_id": guild_id,
        "user_id": target_user_id,
        "role_id": role_id,
        "action": "removed"
    })
    .to_string();
    let event = serde_json::to_string(&ServerMessage {
        message_type: "role_updated".to_string(),
        content: payload,
    })
    .unwrap_or_default();
    hub.broadcast_to_guild_members(db, &guild_id, &event).await;

    Ok(Status::NoContent)
}

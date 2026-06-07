use crate::guilds::{create_guild, create_invite, delete_guild, join_guild, leave_guild, list_user_guilds};
use crate::models::user::AuthenticatedUser;
use rocket::http::Status;
use rocket::serde::json::Json;
use rocket::{State, delete, get, post};
use serde::Deserialize;
use surrealdb::Surreal;
use surrealdb::engine::any::Any;

#[derive(Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct CreateGuildRequest {
    pub name: String,
    pub icon: String,
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
) -> Result<Status, (Status, String)> {
    delete_guild(db, &guild_id, &token.user_id)
        .await
        .map(|_| Status::NoContent)
}

#[post("/<guild_id>/leave", rank = 2)]
pub async fn leave_guild_route(
    token: AuthenticatedUser,
    guild_id: String,
    db: &State<Surreal<Any>>,
) -> Result<Status, (Status, String)> {
    leave_guild(db, &guild_id, &token.user_id)
        .await
        .map(|_| Status::NoContent)
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
) -> Result<(Status, String), (Status, String)> {
    match join_guild(db, &code, &token.user_id).await {
        Ok(guild) => {
            let json = serde_json::to_string(&guild)
                .map_err(|e| (Status::InternalServerError, e.to_string()))?;
            Ok((Status::Ok, json))
        }
        Err(e) => Err(e),
    }
}

use rocket::{get, post};
use rocket::http::Status;
use rocket::serde::json::Json;
use crate::channels;
use crate::models::user::{AuthenticatedUser, CreateDmChannelRequest};
use rocket::State;
use surrealdb::Surreal;
use surrealdb::engine::any::Any;


#[get("/list_dm")]
pub async fn list_dm_channels(
    token: AuthenticatedUser,
    db: &State<Surreal<Any>>,
) -> Result<(Status, String), (Status, String)> {
    match channels::list_channels_for_user(&token.user_id, db).await {
        Ok((channels, friendships)) => {
            let response = serde_json::to_string(&(channels, friendships))
                .map_err(|e| (Status::InternalServerError, e.to_string()))?;
            Ok((Status::Ok, response))
        }
        Err(e) => Err((Status::InternalServerError, e)),
    }
}

#[post("/dm", data = "<body>")]
pub async fn create_dm_channel_route(
    token: AuthenticatedUser,
    db: &State<Surreal<Any>>,
    body: Json<CreateDmChannelRequest>,
) -> Result<(Status, String), (Status, String)> {
    let channel = channels::create_dm_channel(&token.user_id, body.into_inner().recipient_ids, db).await?;
    let response = serde_json::to_string(&channel)
        .map_err(|e| (Status::InternalServerError, e.to_string()))?;
    Ok((Status::Created, response))
}

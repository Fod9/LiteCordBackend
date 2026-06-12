use crate::messages::{assert_channel_access, get_channel_messages};
use crate::models::user::AuthenticatedUser;
use rocket::http::Status;
use rocket::{State, get};
use surrealdb::Surreal;
use surrealdb::engine::any::Any;

#[derive(rocket::FromForm)]
pub struct MessagesQuery {
    #[field(default = 50)]
    pub limit: u32,
    pub before: Option<String>,
}

#[get("/<channel_id>/messages?<query..>")]
pub async fn get_messages(
    channel_id: String,
    query: MessagesQuery,
    token: AuthenticatedUser,
    db: &State<Surreal<Any>>,
) -> Result<(Status, String), (Status, String)> {
    assert_channel_access(db, &channel_id, &token.user_id).await?;

    match get_channel_messages(db, &channel_id, query.limit, query.before).await {
        Ok(messages) => {
            let response = serde_json::to_string(&messages)
                .map_err(|e| (Status::InternalServerError, e.to_string()))?;
            Ok((Status::Ok, response))
        }
        Err(e) => Err((Status::InternalServerError, e)),
    }
}

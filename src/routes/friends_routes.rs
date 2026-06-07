use crate::chat::hub::ChatHub;
use crate::chat::types::ServerMessage;
use crate::friends::{add_friend, list_pending_requests, remove_friend, update_friend_request};
use crate::models::db::{Friendship, FriendshipWithUsers, SimpleUser};
use crate::models::user::AuthenticatedUser;
use rocket::http::Status;
use rocket::serde::json::Json;
use rocket::{State, delete, get, post};
use serde::Serialize;
use surrealdb::Surreal;
use surrealdb::engine::any::Any;
use std::sync::Arc;

#[derive(Serialize)]
struct FriendRequestNotification {
    friendship: Friendship,
    from_user: SimpleUser,
}

async fn fetch_simple_user(db: &Surreal<Any>, user_id: &str) -> Option<SimpleUser> {
    let thing = surrealdb::sql::thing(user_id).ok()?;
    db.query("SELECT name, display_name, id, profile_picture FROM user WHERE id = $id")
        .bind(("id", thing))
        .await
        .ok()?
        .take::<Vec<SimpleUser>>(0)
        .ok()?
        .into_iter()
        .next()
}

#[post("/add_friend/<friend_name>")]
pub async fn add_friend_route(
    token: AuthenticatedUser,
    friend_name: String,
    db: &State<Surreal<Any>>,
    hub: &State<Arc<ChatHub>>,
) -> Result<(Status, String), (Status, String)> {
    let friendship = add_friend(token.user_id.clone(), friend_name, db).await?;

    if let Some(from_user) = fetch_simple_user(db, &friendship.user_a.to_raw()).await {
        let notification = ServerMessage {
            message_type: "friend_request".to_string(),
            content: serde_json::to_string(&FriendRequestNotification {
                friendship: friendship.clone(),
                from_user,
            }).unwrap_or_default(),
        };
        hub.forward_to_client(
            &friendship.user_b.to_raw(),
            &serde_json::to_string(&notification).unwrap_or_default(),
        ).await;
    }

    Ok((Status::Ok, "Demande d'amitié envoyée avec succès.".to_string()))
}

#[post("/update_friend_request/<friendship_id>/<action>")]
pub async fn update_friend_request_route(
    token: AuthenticatedUser,
    friendship_id: String,
    action: String,
    db: &State<Surreal<Any>>,
    hub: &State<Arc<ChatHub>>,
) -> Result<(Status, String), (Status, String)> {
    let friendship = update_friend_request(token.user_id.clone(), friendship_id, db, &action).await?;

    if let Some(from_user) = fetch_simple_user(db, &friendship.user_b.to_raw()).await {
        let notification = ServerMessage {
            message_type: "friend_request_updated".to_string(),
            content: serde_json::to_string(&FriendRequestNotification {
                friendship: friendship.clone(),
                from_user,
            }).unwrap_or_default(),
        };
        hub.forward_to_client(
            &friendship.user_a.to_raw(),
            &serde_json::to_string(&notification).unwrap_or_default(),
        ).await;
    }

    if friendship.status == "accepted" {
        let user_a = friendship.user_a.to_raw();
        let user_b = friendship.user_b.to_raw();
        hub.forward_to_client(&user_a, &hub.presence_event_for(&user_b).await).await;
        hub.forward_to_client(&user_b, &hub.presence_event_for(&user_a).await).await;
    }

    Ok((Status::Ok, format!("Demande d'amitié {}", friendship.status)))
}

#[delete("/<friendship_id>")]
pub async fn remove_friend_route(
    token: AuthenticatedUser,
    friendship_id: String,
    db: &State<Surreal<Any>>,
    hub: &State<Arc<ChatHub>>,
) -> Result<Status, (Status, String)> {
    let friendship_thing = surrealdb::sql::thing(&friendship_id)
        .map_err(|e| (Status::BadRequest, e.to_string()))?;

    let existing: Option<crate::models::db::Friendship> = db
        .query("SELECT * FROM $friendship_id")
        .bind(("friendship_id", friendship_thing))
        .await
        .map_err(|e| (Status::InternalServerError, e.to_string()))?
        .take::<Vec<crate::models::db::Friendship>>(0)
        .map_err(|e| (Status::InternalServerError, e.to_string()))?
        .into_iter()
        .next();

    remove_friend(token.user_id.clone(), friendship_id, db).await?;

    if let Some(friendship) = existing {
        let other_id = if friendship.user_a.to_raw() == token.user_id {
            friendship.user_b.to_raw()
        } else {
            friendship.user_a.to_raw()
        };
        let notification = serde_json::json!({
            "message_type": "friend_removed",
            "content": friendship.id.map(|id| id.to_raw()).unwrap_or_default()
        }).to_string();
        hub.forward_to_client(&other_id, &notification).await;
    }

    Ok(Status::NoContent)
}

#[get("/pending")]
pub async fn list_pending_requests_route(
    token: AuthenticatedUser,
    db: &State<Surreal<Any>>,
) -> Result<Json<Vec<FriendshipWithUsers>>, (Status, String)> {
    match list_pending_requests(token.user_id.clone(), db).await {
        Ok(requests) => Ok(Json(requests)),
        Err(e) => Err((Status::InternalServerError, e)),
    }
}

#[post("/list_friends")]
pub async fn list_friends_route(
    token: AuthenticatedUser,
    db: &State<Surreal<Any>>,
) -> Result<Json<Vec<FriendshipWithUsers>>, (Status, String)> {
    match crate::friends::list_friends(token.user_id.clone(), db).await {
        Ok(friends) => Ok(Json(friends)),
        Err(e) => Err((Status::InternalServerError, e)),
    }
}

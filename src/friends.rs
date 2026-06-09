use crate::models::db::{Friendship, FriendshipWithUsers, SimpleUser, User};
use surrealdb::sql::Thing;
use rocket::http::Status;
use surrealdb::Surreal;
use surrealdb::engine::any::Any;

pub async fn add_friend(
    user_id: String,
    friend_name: String,
    db: &Surreal<Any>,
) -> Result<Friendship, (Status, String)> {
    let target_user = db
        .query("SELECT * FROM user WHERE name = $name")
        .bind(("name", friend_name))
        .await
        .map_err(|e| (Status::InternalServerError, e.to_string()))?
        .take::<Vec<User>>(0)
        .map_err(|e| (Status::InternalServerError, e.to_string()))?
        .into_iter()
        .next()
        .ok_or((Status::NotFound, "Utilisateur cible non trouvé.".to_string()))?;

    let target_id = target_user.id.ok_or((
        Status::InternalServerError,
        "L'utilisateur cible n'a pas d'ID.".to_string(),
    ))?;

    let user_thing = surrealdb::sql::thing(&user_id)
        .map_err(|e| (Status::InternalServerError, e.to_string()))?;

    let existing: Option<Friendship> = db
        .query("SELECT * FROM friendship WHERE (`in` = $a AND out = $b) OR (`in` = $b AND out = $a) LIMIT 1")
        .bind(("a", user_thing.clone()))
        .bind(("b", target_id.clone()))
        .await
        .map_err(|e| (Status::InternalServerError, e.to_string()))?
        .take::<Vec<Friendship>>(0)
        .map_err(|e| (Status::InternalServerError, e.to_string()))?
        .into_iter()
        .next();

    if let Some(friendship) = existing {
        return match friendship.status.as_str() {
            "accepted" => Err((Status::BadRequest, "Vous êtes déjà amis avec cet utilisateur.".to_string())),
            "pending" => Err((Status::BadRequest, "Une demande d'amitié est déjà en attente.".to_string())),
            _ => Err((Status::BadRequest, "Statut d'amitié inconnu.".to_string())),
        };
    }

    let friendship: Option<Friendship> = db
        .query("RELATE $user_a->friendship->$user_b SET status = 'pending', created_at = time::now()")
        .bind(("user_a", user_thing))
        .bind(("user_b", target_id))
        .await
        .map_err(|e| (Status::InternalServerError, e.to_string()))?
        .take(0)
        .map_err(|e| (Status::InternalServerError, e.to_string()))?;

    friendship.ok_or((Status::InternalServerError, "Erreur lors de la création de la demande.".to_string()))
}

pub async fn update_friend_request(
    user_id: String,
    friendship_id: String,
    db: &Surreal<Any>,
    status: &str,
) -> Result<Friendship, (Status, String)> {
    let new_status = match status {
        "accept" => "accepted".to_string(),
        "reject" => "rejected".to_string(),
        _ => return Err((Status::BadRequest, "Statut invalide. Utilisez 'accept' ou 'reject'.".to_string())),
    };

    let user_thing = surrealdb::sql::thing(&user_id)
        .map_err(|e| (Status::InternalServerError, e.to_string()))?;
    let friendship_thing = surrealdb::sql::thing(&friendship_id)
        .map_err(|e| (Status::InternalServerError, e.to_string()))?;

    let existing: Option<Friendship> = db
        .query("SELECT * FROM $friendship_id")
        .bind(("friendship_id", friendship_thing.clone()))
        .await
        .map_err(|e| (Status::InternalServerError, e.to_string()))?
        .take::<Vec<Friendship>>(0)
        .map_err(|e| (Status::InternalServerError, e.to_string()))?
        .into_iter()
        .next();

    let friendship = existing.ok_or((Status::NotFound, "Aucune demande d'amitié trouvée.".to_string()))?;

    if friendship.user_a.to_raw() != user_thing.to_raw() && friendship.user_b.to_raw() != user_thing.to_raw() {
        return Err((Status::Forbidden, "Vous n'êtes pas impliqué dans cette demande d'amitié.".to_string()));
    }

    if friendship.status != "pending" {
        return Err((Status::BadRequest, "Cette demande d'amitié n'est pas en attente.".to_string()));
    }

    let updated: Option<Friendship> = db
        .query("UPDATE $friendship_id SET status = $status")
        .bind(("friendship_id", friendship_thing))
        .bind(("status", new_status))
        .await
        .map_err(|e| (Status::InternalServerError, e.to_string()))?
        .take(0)
        .map_err(|e| (Status::InternalServerError, e.to_string()))?;

    updated.ok_or((Status::InternalServerError, "Erreur lors de la mise à jour.".to_string()))
}

async fn fetch_user(db: &Surreal<Any>, id: &Thing) -> Option<SimpleUser> {
    db.query("SELECT id, name, display_name, profile_picture FROM user WHERE id = $id")
        .bind(("id", id.clone()))
        .await
        .ok()?
        .take::<Vec<SimpleUser>>(0)
        .ok()?
        .into_iter()
        .next()
}

async fn enrich_friendships(db: &Surreal<Any>, friendships: Vec<Friendship>) -> Result<Vec<FriendshipWithUsers>, String> {
    let mut result = Vec::with_capacity(friendships.len());
    for f in friendships {
        let in_user = fetch_user(db, &f.user_a).await
            .ok_or_else(|| format!("User not found: {}", f.user_a.to_raw()))?;
        let out_user = fetch_user(db, &f.user_b).await
            .ok_or_else(|| format!("User not found: {}", f.user_b.to_raw()))?;
        result.push(FriendshipWithUsers {
            id: f.id,
            in_user,
            out_user,
            status: f.status,
            created_at: f.created_at,
        });
    }
    Ok(result)
}

pub async fn list_friends(
    user_id: String,
    db: &Surreal<Any>,
) -> Result<Vec<FriendshipWithUsers>, String> {
    let user_thing = surrealdb::sql::thing(&user_id).map_err(|e| e.to_string())?;

    let friendships = db
        .query("SELECT * FROM friendship WHERE (`in` = $user_id OR out = $user_id) AND status = 'accepted'")
        .bind(("user_id", user_thing))
        .await
        .map_err(|e| e.to_string())?
        .take::<Vec<Friendship>>(0)
        .map_err(|e| e.to_string())?;

    enrich_friendships(db, friendships).await
}

pub async fn are_accepted_friends(
    db: &Surreal<Any>,
    user_a: &str,
    user_b: &str,
) -> bool {
    let Ok(thing_a) = surrealdb::sql::thing(user_a) else { return false; };
    let Ok(thing_b) = surrealdb::sql::thing(user_b) else { return false; };

    match db
        .query("SELECT * FROM friendship WHERE ((`in` = $a AND out = $b) OR (`in` = $b AND out = $a)) AND status = 'accepted' LIMIT 1")
        .bind(("a", thing_a))
        .bind(("b", thing_b))
        .await
    {
        Ok(mut res) => !res.take::<Vec<Friendship>>(0).unwrap_or_default().is_empty(),
        Err(_) => false,
    }
}

pub async fn remove_friend(
    user_id: String,
    friendship_id: String,
    db: &Surreal<Any>,
) -> Result<(), (Status, String)> {
    let user_thing = surrealdb::sql::thing(&user_id)
        .map_err(|e| (Status::InternalServerError, e.to_string()))?;
    let friendship_thing = surrealdb::sql::thing(&friendship_id)
        .map_err(|e| (Status::InternalServerError, e.to_string()))?;

    let existing: Option<Friendship> = db
        .query("SELECT * FROM $friendship_id")
        .bind(("friendship_id", friendship_thing.clone()))
        .await
        .map_err(|e| (Status::InternalServerError, e.to_string()))?
        .take::<Vec<Friendship>>(0)
        .map_err(|e| (Status::InternalServerError, e.to_string()))?
        .into_iter()
        .next();

    let friendship = existing.ok_or((Status::NotFound, "Amitié introuvable.".to_string()))?;

    if friendship.user_a.to_raw() != user_thing.to_raw() && friendship.user_b.to_raw() != user_thing.to_raw() {
        return Err((Status::Forbidden, "Vous n'êtes pas impliqué dans cette amitié.".to_string()));
    }

    db.query("DELETE $friendship_id")
        .bind(("friendship_id", friendship_thing))
        .await
        .map_err(|e| (Status::InternalServerError, e.to_string()))?;

    Ok(())
}

pub async fn list_pending_requests(
    user_id: String,
    db: &Surreal<Any>,
) -> Result<Vec<FriendshipWithUsers>, String> {
    let user_thing = surrealdb::sql::thing(&user_id).map_err(|e| e.to_string())?;

    let friendships = db
        .query("SELECT * FROM friendship WHERE out = $user_id AND status = 'pending'")
        .bind(("user_id", user_thing))
        .await
        .map_err(|e| e.to_string())?
        .take::<Vec<Friendship>>(0)
        .map_err(|e| e.to_string())?;

    enrich_friendships(db, friendships).await
}

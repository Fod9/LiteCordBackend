use crate::models::db::{Guild, MemberOf, Role};
use rocket::http::Status;
use surrealdb::Surreal;
use surrealdb::engine::any::Any;
use surrealdb::sql::Thing;

async fn get_guild_owner(
    db: &Surreal<Any>,
    guild_id: &Thing,
) -> Result<Thing, (Status, String)> {
    let guild: Option<Guild> = db
        .query("SELECT * FROM $guild_id")
        .bind(("guild_id", guild_id.clone()))
        .await
        .map_err(|e| (Status::InternalServerError, e.to_string()))?
        .take(0)
        .map_err(|e| (Status::InternalServerError, e.to_string()))?;

    guild
        .ok_or((Status::NotFound, "Guild not found".to_string()))
        .map(|g| g.owner)
}

pub async fn create_role(
    db: &Surreal<Any>,
    guild_id: &str,
    user_id: &str,
    name: String,
    color: String,
    position: i32,
    permissions: Vec<String>,
) -> Result<Role, (Status, String)> {
    let guild_thing = surrealdb::sql::thing(guild_id)
        .map_err(|e| (Status::BadRequest, e.to_string()))?;
    let user_thing = surrealdb::sql::thing(user_id)
        .map_err(|e| (Status::BadRequest, e.to_string()))?;

    let owner = get_guild_owner(db, &guild_thing).await?;
    if owner.to_raw() != user_thing.to_raw() {
        return Err((Status::Forbidden, "Only the guild owner can create roles".to_string()));
    }

    let role: Option<Role> = db
        .query(
            "CREATE role SET guild = $guild, name = $name, color = $color, position = $position, permissions = $permissions",
        )
        .bind(("guild", guild_thing))
        .bind(("name", name))
        .bind(("color", color))
        .bind(("position", position))
        .bind(("permissions", permissions))
        .await
        .map_err(|e| (Status::InternalServerError, e.to_string()))?
        .take(0)
        .map_err(|e| (Status::InternalServerError, e.to_string()))?;

    role.ok_or((Status::InternalServerError, "Failed to create role".to_string()))
}

pub async fn delete_role(
    db: &Surreal<Any>,
    guild_id: &str,
    role_id: &str,
    user_id: &str,
) -> Result<(), (Status, String)> {
    let guild_thing = surrealdb::sql::thing(guild_id)
        .map_err(|e| (Status::BadRequest, e.to_string()))?;
    let role_thing = surrealdb::sql::thing(role_id)
        .map_err(|e| (Status::BadRequest, e.to_string()))?;
    let user_thing = surrealdb::sql::thing(user_id)
        .map_err(|e| (Status::BadRequest, e.to_string()))?;

    let owner = get_guild_owner(db, &guild_thing).await?;
    if owner.to_raw() != user_thing.to_raw() {
        return Err((Status::Forbidden, "Only the guild owner can delete roles".to_string()));
    }

    db.query("UPDATE member_of SET roles -= $role WHERE out = $guild")
        .bind(("role", role_thing.clone()))
        .bind(("guild", guild_thing))
        .await
        .map_err(|e| (Status::InternalServerError, e.to_string()))?;

    db.query("DELETE $role_id")
        .bind(("role_id", role_thing))
        .await
        .map_err(|e| (Status::InternalServerError, e.to_string()))?;

    Ok(())
}

pub async fn list_roles(db: &Surreal<Any>, guild_id: &str) -> Result<Vec<Role>, (Status, String)> {
    let guild_thing = surrealdb::sql::thing(guild_id)
        .map_err(|e| (Status::BadRequest, e.to_string()))?;

    db.query("SELECT * FROM role WHERE guild = $guild ORDER BY position ASC")
        .bind(("guild", guild_thing))
        .await
        .map_err(|e| (Status::InternalServerError, e.to_string()))?
        .take(0)
        .map_err(|e| (Status::InternalServerError, e.to_string()))
}

pub async fn assign_role(
    db: &Surreal<Any>,
    guild_id: &str,
    role_id: &str,
    target_user_id: &str,
    user_id: &str,
) -> Result<(), (Status, String)> {
    let guild_thing = surrealdb::sql::thing(guild_id)
        .map_err(|e| (Status::BadRequest, e.to_string()))?;
    let role_thing = surrealdb::sql::thing(role_id)
        .map_err(|e| (Status::BadRequest, e.to_string()))?;
    let target_thing = surrealdb::sql::thing(target_user_id)
        .map_err(|e| (Status::BadRequest, e.to_string()))?;
    let user_thing = surrealdb::sql::thing(user_id)
        .map_err(|e| (Status::BadRequest, e.to_string()))?;

    let owner = get_guild_owner(db, &guild_thing).await?;
    if owner.to_raw() != user_thing.to_raw() {
        return Err((Status::Forbidden, "Only the guild owner can assign roles".to_string()));
    }

    let membership: Vec<MemberOf> = db
        .query("SELECT * FROM member_of WHERE `in` = $user AND out = $guild")
        .bind(("user", target_thing.clone()))
        .bind(("guild", guild_thing.clone()))
        .await
        .map_err(|e| (Status::InternalServerError, e.to_string()))?
        .take(0)
        .map_err(|e| (Status::InternalServerError, e.to_string()))?;

    if membership.is_empty() {
        return Err((Status::NotFound, "Target user is not a member of this guild".to_string()));
    }

    db.query("UPDATE member_of SET roles += $role WHERE `in` = $user AND out = $guild")
        .bind(("role", role_thing))
        .bind(("user", target_thing))
        .bind(("guild", guild_thing))
        .await
        .map_err(|e| (Status::InternalServerError, e.to_string()))?;

    Ok(())
}

pub async fn remove_role(
    db: &Surreal<Any>,
    guild_id: &str,
    role_id: &str,
    target_user_id: &str,
    user_id: &str,
) -> Result<(), (Status, String)> {
    let guild_thing = surrealdb::sql::thing(guild_id)
        .map_err(|e| (Status::BadRequest, e.to_string()))?;
    let role_thing = surrealdb::sql::thing(role_id)
        .map_err(|e| (Status::BadRequest, e.to_string()))?;
    let target_thing = surrealdb::sql::thing(target_user_id)
        .map_err(|e| (Status::BadRequest, e.to_string()))?;
    let user_thing = surrealdb::sql::thing(user_id)
        .map_err(|e| (Status::BadRequest, e.to_string()))?;

    let owner = get_guild_owner(db, &guild_thing).await?;
    if owner.to_raw() != user_thing.to_raw() {
        return Err((Status::Forbidden, "Only the guild owner can remove roles".to_string()));
    }

    db.query("UPDATE member_of SET roles -= $role WHERE `in` = $user AND out = $guild")
        .bind(("role", role_thing))
        .bind(("user", target_thing))
        .bind(("guild", guild_thing))
        .await
        .map_err(|e| (Status::InternalServerError, e.to_string()))?;

    Ok(())
}

pub async fn check_permission(
    db: &Surreal<Any>,
    guild_id: &str,
    user_id: &str,
    permission: &str,
) -> Result<bool, String> {
    let guild_thing = surrealdb::sql::thing(guild_id).map_err(|e| e.to_string())?;
    let user_thing = surrealdb::sql::thing(user_id).map_err(|e| e.to_string())?;

    let memberships: Vec<MemberOf> = db
        .query("SELECT * FROM member_of WHERE `in` = $user AND out = $guild")
        .bind(("user", user_thing))
        .bind(("guild", guild_thing))
        .await
        .map_err(|e| e.to_string())?
        .take(0)
        .map_err(|e| e.to_string())?;

    let membership = match memberships.into_iter().next() {
        Some(m) => m,
        None => return Ok(false),
    };

    if membership.roles.is_empty() {
        return Ok(false);
    }

    let roles: Vec<Role> = db
        .query("SELECT * FROM $role_ids")
        .bind(("role_ids", membership.roles))
        .await
        .map_err(|e| e.to_string())?
        .take(0)
        .map_err(|e| e.to_string())?;

    Ok(roles.iter().any(|r| r.permissions.iter().any(|p| p == permission)))
}

use crate::models::db::{MemberOf, Role};
use crate::permissions::{
    MemberPermissions, get_member_permissions, is_known_permission, member_highest_position,
    require_permission, role_hierarchy_error, unknown_permissions, unknown_permissions_error,
};
use rocket::http::Status;
use surrealdb::Surreal;
use surrealdb::engine::any::Any;
use surrealdb::sql::Thing;

async fn get_guild_role(
    db: &Surreal<Any>,
    guild_thing: &Thing,
    role_thing: &Thing,
) -> Result<Role, (Status, String)> {
    let role: Option<Role> = db
        .query("SELECT * FROM $role_id WHERE guild = $guild_id")
        .bind(("role_id", role_thing.clone()))
        .bind(("guild_id", guild_thing.clone()))
        .await
        .map_err(|e| (Status::InternalServerError, e.to_string()))?
        .take(0)
        .map_err(|e| (Status::InternalServerError, e.to_string()))?;

    role.ok_or((Status::NotFound, "Role not found or does not belong to this guild".to_string()))
}

fn validate_vocabulary(permissions: &[String]) -> Result<(), (Status, String)> {
    let unknown = unknown_permissions(permissions);
    if !unknown.is_empty() {
        return Err(unknown_permissions_error(&unknown));
    }
    Ok(())
}

fn assert_can_grant(
    actor: &MemberPermissions,
    current: &[String],
    requested: &[String],
) -> Result<(), (Status, String)> {
    let added: Vec<String> = requested
        .iter()
        .filter(|p| !current.contains(p))
        .cloned()
        .collect();
    if !actor.can_grant(&added) {
        return Err(role_hierarchy_error());
    }
    Ok(())
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

    validate_vocabulary(&permissions)?;

    let actor = require_permission(db, guild_id, user_id, "manage_roles").await?;
    if !actor.can_act_on_position(position) {
        return Err(role_hierarchy_error());
    }
    assert_can_grant(&actor, &[], &permissions)?;

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

pub async fn update_role(
    db: &Surreal<Any>,
    guild_id: &str,
    role_id: &str,
    user_id: &str,
    name: Option<String>,
    color: Option<String>,
    position: Option<i32>,
    permissions: Option<Vec<String>>,
) -> Result<Role, (Status, String)> {
    let guild_thing = surrealdb::sql::thing(guild_id)
        .map_err(|e| (Status::BadRequest, e.to_string()))?;
    let role_thing = surrealdb::sql::thing(role_id)
        .map_err(|e| (Status::BadRequest, e.to_string()))?;

    if let Some(perms) = &permissions {
        validate_vocabulary(perms)?;
    }

    let actor = require_permission(db, guild_id, user_id, "manage_roles").await?;
    let role = get_guild_role(db, &guild_thing, &role_thing).await?;

    if !actor.can_act_on_position(role.position) {
        return Err(role_hierarchy_error());
    }

    let new_position = position.unwrap_or(role.position);
    if !actor.can_act_on_position(new_position) {
        return Err(role_hierarchy_error());
    }

    // Unknown values left in storage are purged on every PATCH.
    let current: Vec<String> = role
        .permissions
        .iter()
        .filter(|p| is_known_permission(p))
        .cloned()
        .collect();

    let new_permissions = match permissions {
        Some(requested) => {
            assert_can_grant(&actor, &current, &requested)?;
            requested
        }
        None => current,
    };

    let updated: Option<Role> = db
        .query("UPDATE $role_id SET name = $name, color = $color, position = $position, permissions = $permissions")
        .bind(("role_id", role_thing))
        .bind(("name", name.unwrap_or(role.name)))
        .bind(("color", color.unwrap_or(role.color)))
        .bind(("position", new_position))
        .bind(("permissions", new_permissions))
        .await
        .map_err(|e| (Status::InternalServerError, e.to_string()))?
        .take(0)
        .map_err(|e| (Status::InternalServerError, e.to_string()))?;

    updated.ok_or((Status::InternalServerError, "Failed to update role".to_string()))
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

    let actor = require_permission(db, guild_id, user_id, "manage_roles").await?;
    let role = get_guild_role(db, &guild_thing, &role_thing).await?;

    if !actor.can_act_on_position(role.position) {
        return Err(role_hierarchy_error());
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

pub async fn list_roles(
    db: &Surreal<Any>,
    guild_id: &str,
    user_id: &str,
) -> Result<Vec<Role>, (Status, String)> {
    let guild_thing = surrealdb::sql::thing(guild_id)
        .map_err(|e| (Status::BadRequest, e.to_string()))?;

    get_member_permissions(db, guild_id, user_id).await?;

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

    let actor = require_permission(db, guild_id, user_id, "manage_roles").await?;
    let role = get_guild_role(db, &guild_thing, &role_thing).await?;

    if !actor.can_act_on_position(role.position) {
        return Err(role_hierarchy_error());
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

    let actor = require_permission(db, guild_id, user_id, "manage_roles").await?;
    let role = get_guild_role(db, &guild_thing, &role_thing).await?;

    if !actor.can_act_on_position(role.position) {
        return Err(role_hierarchy_error());
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
    match get_member_permissions(db, guild_id, user_id).await {
        Ok(perms) => Ok(perms.has(permission)),
        Err((status, e)) => {
            if status == Status::Forbidden || status == Status::NotFound {
                Ok(false)
            } else {
                Err(e)
            }
        }
    }
}

pub async fn can_kick(
    db: &Surreal<Any>,
    guild_thing: &Thing,
    actor: &MemberPermissions,
    target_thing: &Thing,
) -> Result<bool, (Status, String)> {
    if actor.bypass {
        return Ok(true);
    }
    let target_position = member_highest_position(db, guild_thing, target_thing).await?;
    Ok(actor.highest_position < target_position)
}

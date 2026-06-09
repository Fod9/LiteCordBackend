use crate::models::db::{Guild, GuildInvite, MemberOf, MemberProfile, SimpleUser};
use rand::Rng;
use rocket::http::Status;
use surrealdb::Surreal;
use surrealdb::engine::any::Any;
use surrealdb::sql::Thing;

fn generate_invite_code() -> String {
    rand::thread_rng()
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(8)
        .map(char::from)
        .collect()
}

pub async fn create_guild(
    db: &Surreal<Any>,
    owner_id: &str,
    name: String,
    icon: String,
) -> Result<Guild, String> {
    let owner_thing = surrealdb::sql::thing(owner_id).map_err(|e| e.to_string())?;

    let mut res = db
        .query("CREATE guild SET name = $name, icon = $icon, owner = $owner, created_at = time::now()")
        .bind(("name", name))
        .bind(("icon", icon))
        .bind(("owner", owner_thing.clone()))
        .await
        .map_err(|e| e.to_string())?;

    let guild: Option<Guild> = res.take(0).map_err(|e| e.to_string())?;
    let guild = guild.ok_or_else(|| "Failed to create guild".to_string())?;

    let guild_thing = guild.id.clone().ok_or_else(|| "Guild has no ID".to_string())?;

    db.query("RELATE $user->member_of->$guild SET roles = [], nickname = NONE")
        .bind(("user", owner_thing))
        .bind(("guild", guild_thing))
        .await
        .map_err(|e| e.to_string())?;

    Ok(guild)
}

pub async fn delete_guild(
    db: &Surreal<Any>,
    guild_id: &str,
    user_id: &str,
) -> Result<Vec<String>, (Status, String)> {
    let guild_thing = surrealdb::sql::thing(guild_id)
        .map_err(|e| (Status::BadRequest, e.to_string()))?;
    let user_thing = surrealdb::sql::thing(user_id)
        .map_err(|e| (Status::BadRequest, e.to_string()))?;

    let guild: Option<Guild> = db
        .query("SELECT * FROM $guild_id")
        .bind(("guild_id", guild_thing.clone()))
        .await
        .map_err(|e| (Status::InternalServerError, e.to_string()))?
        .take(0)
        .map_err(|e| (Status::InternalServerError, e.to_string()))?;

    let guild = guild.ok_or((Status::NotFound, "Guild not found".to_string()))?;

    if guild.owner.to_raw() != user_thing.to_raw() {
        return Err((Status::Forbidden, "Only the owner can delete the guild".to_string()));
    }

    let members: Vec<MemberOf> = db
        .query("SELECT * FROM member_of WHERE out = $guild_id")
        .bind(("guild_id", guild_thing.clone()))
        .await
        .map_err(|e| (Status::InternalServerError, e.to_string()))?
        .take(0)
        .map_err(|e| (Status::InternalServerError, e.to_string()))?;

    let member_ids: Vec<String> = members.iter().map(|m| m.user.to_raw()).collect();

    db.query("DELETE member_of WHERE out = $guild_id")
        .bind(("guild_id", guild_thing.clone()))
        .await
        .map_err(|e| (Status::InternalServerError, e.to_string()))?;

    db.query("DELETE guild_invite WHERE guild = $guild_id")
        .bind(("guild_id", guild_thing.clone()))
        .await
        .map_err(|e| (Status::InternalServerError, e.to_string()))?;

    db.query("DELETE $guild_id")
        .bind(("guild_id", guild_thing))
        .await
        .map_err(|e| (Status::InternalServerError, e.to_string()))?;

    Ok(member_ids)
}

pub async fn leave_guild(
    db: &Surreal<Any>,
    guild_id: &str,
    user_id: &str,
) -> Result<(), (Status, String)> {
    let guild_thing = surrealdb::sql::thing(guild_id)
        .map_err(|e| (Status::BadRequest, e.to_string()))?;
    let user_thing = surrealdb::sql::thing(user_id)
        .map_err(|e| (Status::BadRequest, e.to_string()))?;

    let guild: Option<Guild> = db
        .query("SELECT * FROM $guild_id")
        .bind(("guild_id", guild_thing.clone()))
        .await
        .map_err(|e| (Status::InternalServerError, e.to_string()))?
        .take(0)
        .map_err(|e| (Status::InternalServerError, e.to_string()))?;

    let guild = guild.ok_or((Status::NotFound, "Guild not found".to_string()))?;

    if guild.owner.to_raw() == user_thing.to_raw() {
        return Err((Status::BadRequest, "The owner cannot leave the guild. Delete it instead.".to_string()));
    }

    let membership: Vec<MemberOf> = db
        .query("SELECT * FROM member_of WHERE `in` = $user_id AND out = $guild_id")
        .bind(("user_id", user_thing.clone()))
        .bind(("guild_id", guild_thing.clone()))
        .await
        .map_err(|e| (Status::InternalServerError, e.to_string()))?
        .take(0)
        .map_err(|e| (Status::InternalServerError, e.to_string()))?;

    if membership.is_empty() {
        return Err((Status::NotFound, "User is not a member of this guild".to_string()));
    }

    db.query("DELETE member_of WHERE `in` = $user_id AND out = $guild_id")
        .bind(("user_id", user_thing))
        .bind(("guild_id", guild_thing))
        .await
        .map_err(|e| (Status::InternalServerError, e.to_string()))?;

    Ok(())
}

pub async fn list_user_guilds(
    db: &Surreal<Any>,
    user_id: &str,
) -> Result<Vec<Guild>, String> {
    let user_thing: Thing = surrealdb::sql::thing(user_id).map_err(|e| e.to_string())?;

    let memberships: Vec<MemberOf> = db
        .query("SELECT * FROM member_of WHERE `in` = $user_id")
        .bind(("user_id", user_thing))
        .await
        .map_err(|e| e.to_string())?
        .take(0)
        .map_err(|e| e.to_string())?;

    if memberships.is_empty() {
        return Ok(vec![]);
    }

    let guild_ids: Vec<Thing> = memberships.into_iter().map(|m| m.guild).collect();

    db.query("SELECT * FROM guild WHERE id IN $guild_ids")
        .bind(("guild_ids", guild_ids))
        .await
        .map_err(|e| e.to_string())?
        .take(0)
        .map_err(|e| e.to_string())
}

pub async fn create_invite(
    db: &Surreal<Any>,
    guild_id: &str,
    inviter_id: &str,
) -> Result<GuildInvite, (Status, String)> {
    let guild_thing = surrealdb::sql::thing(guild_id)
        .map_err(|e| (Status::BadRequest, e.to_string()))?;
    let inviter_thing = surrealdb::sql::thing(inviter_id)
        .map_err(|e| (Status::BadRequest, e.to_string()))?;

    let membership: Vec<MemberOf> = db
        .query("SELECT * FROM member_of WHERE `in` = $user_id AND out = $guild_id")
        .bind(("user_id", inviter_thing.clone()))
        .bind(("guild_id", guild_thing.clone()))
        .await
        .map_err(|e| (Status::InternalServerError, e.to_string()))?
        .take(0)
        .map_err(|e| (Status::InternalServerError, e.to_string()))?;

    if membership.is_empty() {
        return Err((Status::Forbidden, "You must be a member to create an invite".to_string()));
    }

    let code = generate_invite_code();

    let invite: Option<GuildInvite> = db
        .query("CREATE guild_invite SET guild = $guild, inviter = $inviter, code = $code, created_at = time::now()")
        .bind(("guild", guild_thing))
        .bind(("inviter", inviter_thing))
        .bind(("code", code))
        .await
        .map_err(|e| (Status::InternalServerError, e.to_string()))?
        .take(0)
        .map_err(|e| (Status::InternalServerError, e.to_string()))?;

    invite.ok_or((Status::InternalServerError, "Failed to create invite".to_string()))
}

pub async fn join_guild_directly(
    db: &Surreal<Any>,
    guild_id: &Thing,
    user_id: &Thing,
) -> Result<(), String> {
    db.query("RELATE $user->member_of->$guild SET roles = [], nickname = NONE")
        .bind(("user", user_id.clone()))
        .bind(("guild", guild_id.clone()))
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

async fn fetch_simple_user(db: &Surreal<Any>, user_id: &Thing) -> Option<SimpleUser> {
    db.query("SELECT id, name, display_name, profile_picture FROM user WHERE id = $id")
        .bind(("id", user_id.clone()))
        .await
        .ok()?
        .take::<Vec<SimpleUser>>(0)
        .ok()?
        .into_iter()
        .next()
}

pub async fn list_guild_members(
    db: &Surreal<Any>,
    guild_id: &str,
    user_id: &str,
) -> Result<Vec<MemberProfile>, (Status, String)> {
    let guild_thing = surrealdb::sql::thing(guild_id)
        .map_err(|e| (Status::BadRequest, e.to_string()))?;
    let user_thing = surrealdb::sql::thing(user_id)
        .map_err(|e| (Status::BadRequest, e.to_string()))?;

    let caller_membership: Vec<MemberOf> = db
        .query("SELECT * FROM member_of WHERE `in` = $user_id AND out = $guild_id")
        .bind(("user_id", user_thing))
        .bind(("guild_id", guild_thing.clone()))
        .await
        .map_err(|e| (Status::InternalServerError, e.to_string()))?
        .take(0)
        .map_err(|e| (Status::InternalServerError, e.to_string()))?;

    if caller_membership.is_empty() {
        return Err((Status::Forbidden, "You are not a member of this guild".to_string()));
    }

    let members: Vec<MemberOf> = db
        .query("SELECT * FROM member_of WHERE out = $guild_id")
        .bind(("guild_id", guild_thing))
        .await
        .map_err(|e| (Status::InternalServerError, e.to_string()))?
        .take(0)
        .map_err(|e| (Status::InternalServerError, e.to_string()))?;

    let mut profiles = Vec::with_capacity(members.len());
    for m in members {
        let user = fetch_simple_user(db, &m.user).await.ok_or_else(|| {
            (Status::InternalServerError, format!("User not found: {}", m.user.to_raw()))
        })?;
        profiles.push(MemberProfile {
            id: m.id,
            user,
            roles: m.roles,
            nickname: m.nickname,
            joined_at: m.joined_at,
        });
    }

    Ok(profiles)
}

pub async fn kick_member(
    db: &Surreal<Any>,
    guild_id: &str,
    target_user_id: &str,
    requester_id: &str,
) -> Result<(), (Status, String)> {
    let guild_thing = surrealdb::sql::thing(guild_id)
        .map_err(|e| (Status::BadRequest, e.to_string()))?;
    let target_thing = surrealdb::sql::thing(target_user_id)
        .map_err(|e| (Status::BadRequest, e.to_string()))?;
    let requester_thing = surrealdb::sql::thing(requester_id)
        .map_err(|e| (Status::BadRequest, e.to_string()))?;

    let guild: Option<Guild> = db
        .query("SELECT * FROM $guild_id")
        .bind(("guild_id", guild_thing.clone()))
        .await
        .map_err(|e| (Status::InternalServerError, e.to_string()))?
        .take(0)
        .map_err(|e| (Status::InternalServerError, e.to_string()))?;

    let guild = guild.ok_or((Status::NotFound, "Guild not found".to_string()))?;

    if guild.owner.to_raw() != requester_thing.to_raw() {
        return Err((Status::Forbidden, "Only the owner can kick members".to_string()));
    }

    if guild.owner.to_raw() == target_thing.to_raw() {
        return Err((Status::BadRequest, "Cannot kick the guild owner".to_string()));
    }

    let membership: Vec<MemberOf> = db
        .query("SELECT * FROM member_of WHERE `in` = $user_id AND out = $guild_id")
        .bind(("user_id", target_thing.clone()))
        .bind(("guild_id", guild_thing.clone()))
        .await
        .map_err(|e| (Status::InternalServerError, e.to_string()))?
        .take(0)
        .map_err(|e| (Status::InternalServerError, e.to_string()))?;

    if membership.is_empty() {
        return Err((Status::NotFound, "User is not a member of this guild".to_string()));
    }

    db.query("DELETE member_of WHERE `in` = $user_id AND out = $guild_id")
        .bind(("user_id", target_thing))
        .bind(("guild_id", guild_thing))
        .await
        .map_err(|e| (Status::InternalServerError, e.to_string()))?;

    Ok(())
}

pub async fn update_guild(
    db: &Surreal<Any>,
    guild_id: &str,
    user_id: &str,
    name: Option<String>,
    icon: Option<String>,
) -> Result<Guild, (Status, String)> {
    let guild_thing = surrealdb::sql::thing(guild_id)
        .map_err(|e| (Status::BadRequest, e.to_string()))?;
    let user_thing = surrealdb::sql::thing(user_id)
        .map_err(|e| (Status::BadRequest, e.to_string()))?;

    let guild: Option<Guild> = db
        .query("SELECT * FROM $guild_id")
        .bind(("guild_id", guild_thing.clone()))
        .await
        .map_err(|e| (Status::InternalServerError, e.to_string()))?
        .take(0)
        .map_err(|e| (Status::InternalServerError, e.to_string()))?;

    let guild = guild.ok_or((Status::NotFound, "Guild not found".to_string()))?;

    if guild.owner.to_raw() != user_thing.to_raw() {
        return Err((Status::Forbidden, "Only the owner can update the guild".to_string()));
    }

    let updated: Option<Guild> = db
        .query("UPDATE $guild_id SET name = $name ?? name, icon = $icon ?? icon")
        .bind(("guild_id", guild_thing))
        .bind(("name", name))
        .bind(("icon", icon))
        .await
        .map_err(|e| (Status::InternalServerError, e.to_string()))?
        .take(0)
        .map_err(|e| (Status::InternalServerError, e.to_string()))?;

    updated.ok_or((Status::InternalServerError, "Failed to update guild".to_string()))
}

pub async fn list_guild_invites(
    db: &Surreal<Any>,
    guild_id: &str,
    user_id: &str,
) -> Result<Vec<GuildInvite>, (Status, String)> {
    let guild_thing = surrealdb::sql::thing(guild_id)
        .map_err(|e| (Status::BadRequest, e.to_string()))?;
    let user_thing = surrealdb::sql::thing(user_id)
        .map_err(|e| (Status::BadRequest, e.to_string()))?;

    let guild: Option<Guild> = db
        .query("SELECT * FROM $guild_id")
        .bind(("guild_id", guild_thing.clone()))
        .await
        .map_err(|e| (Status::InternalServerError, e.to_string()))?
        .take(0)
        .map_err(|e| (Status::InternalServerError, e.to_string()))?;

    let guild = guild.ok_or((Status::NotFound, "Guild not found".to_string()))?;

    if guild.owner.to_raw() != user_thing.to_raw() {
        return Err((Status::Forbidden, "Only the owner can list invites".to_string()));
    }

    db.query("SELECT * FROM guild_invite WHERE guild = $guild_id")
        .bind(("guild_id", guild_thing))
        .await
        .map_err(|e| (Status::InternalServerError, e.to_string()))?
        .take(0)
        .map_err(|e| (Status::InternalServerError, e.to_string()))
}

pub async fn revoke_invite(
    db: &Surreal<Any>,
    guild_id: &str,
    invite_id: &str,
    user_id: &str,
) -> Result<(), (Status, String)> {
    let guild_thing = surrealdb::sql::thing(guild_id)
        .map_err(|e| (Status::BadRequest, e.to_string()))?;
    let invite_thing = surrealdb::sql::thing(invite_id)
        .map_err(|e| (Status::BadRequest, e.to_string()))?;
    let user_thing = surrealdb::sql::thing(user_id)
        .map_err(|e| (Status::BadRequest, e.to_string()))?;

    let guild: Option<Guild> = db
        .query("SELECT * FROM $guild_id")
        .bind(("guild_id", guild_thing.clone()))
        .await
        .map_err(|e| (Status::InternalServerError, e.to_string()))?
        .take(0)
        .map_err(|e| (Status::InternalServerError, e.to_string()))?;

    let guild = guild.ok_or((Status::NotFound, "Guild not found".to_string()))?;

    if guild.owner.to_raw() != user_thing.to_raw() {
        return Err((Status::Forbidden, "Only the owner can revoke invites".to_string()));
    }

    let invite: Option<GuildInvite> = db
        .query("SELECT * FROM $invite_id WHERE guild = $guild_id")
        .bind(("invite_id", invite_thing.clone()))
        .bind(("guild_id", guild_thing))
        .await
        .map_err(|e| (Status::InternalServerError, e.to_string()))?
        .take(0)
        .map_err(|e| (Status::InternalServerError, e.to_string()))?;

    invite.ok_or((Status::NotFound, "Invite not found or does not belong to this guild".to_string()))?;

    db.query("DELETE $invite_id")
        .bind(("invite_id", invite_thing))
        .await
        .map_err(|e| (Status::InternalServerError, e.to_string()))?;

    Ok(())
}

pub async fn join_guild(
    db: &Surreal<Any>,
    invite_code: &str,
    user_id: &str,
) -> Result<Guild, (Status, String)> {
    let user_thing = surrealdb::sql::thing(user_id)
        .map_err(|e| (Status::BadRequest, e.to_string()))?;

    let invite: Option<GuildInvite> = db
        .query("SELECT * FROM guild_invite WHERE code = $code AND (expires_at IS NONE OR expires_at > time::now())")
        .bind(("code", invite_code.to_string()))
        .await
        .map_err(|e| (Status::InternalServerError, e.to_string()))?
        .take(0)
        .map_err(|e| (Status::InternalServerError, e.to_string()))?;

    let invite = invite.ok_or((Status::NotFound, "Invalid or expired invite code".to_string()))?;

    let guild_thing = invite.guild.clone();

    let existing: Vec<MemberOf> = db
        .query("SELECT * FROM member_of WHERE `in` = $user_id AND out = $guild_id")
        .bind(("user_id", user_thing.clone()))
        .bind(("guild_id", guild_thing.clone()))
        .await
        .map_err(|e| (Status::InternalServerError, e.to_string()))?
        .take(0)
        .map_err(|e| (Status::InternalServerError, e.to_string()))?;

    if !existing.is_empty() {
        return Err((Status::Conflict, "Already a member of this guild".to_string()));
    }

    db.query("RELATE $user->member_of->$guild SET roles = [], nickname = NONE")
        .bind(("user", user_thing))
        .bind(("guild", guild_thing.clone()))
        .await
        .map_err(|e| (Status::InternalServerError, e.to_string()))?;

    let guild: Option<Guild> = db
        .query("SELECT * FROM $guild_id")
        .bind(("guild_id", guild_thing))
        .await
        .map_err(|e| (Status::InternalServerError, e.to_string()))?
        .take(0)
        .map_err(|e| (Status::InternalServerError, e.to_string()))?;

    guild.ok_or((Status::InternalServerError, "Guild not found after join".to_string()))
}

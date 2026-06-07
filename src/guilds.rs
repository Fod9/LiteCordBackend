use crate::models::db::{Guild, GuildInvite, MemberOf};
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
        .query("CREATE guild SET name = $name, icon = $icon, owner = $owner")
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

    if guild.owner.to_raw() != user_thing.to_raw() {
        return Err((Status::Forbidden, "Only the owner can delete the guild".to_string()));
    }

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

    Ok(())
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
        .query("CREATE guild_invite SET guild = $guild, inviter = $inviter, code = $code")
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

use crate::models::db::{Channel, ChannelType, Guild, MemberOf, PermissionOverwrite, Role};
use rocket::http::Status;
use std::collections::HashSet;
use surrealdb::Surreal;
use surrealdb::engine::any::Any;
use surrealdb::sql::Thing;

pub const ALL_PERMISSIONS: &[&str] = &[
    "administrator",
    "manage_guild",
    "manage_roles",
    "manage_channels",
    "create_invite",
    "manage_invites",
    "kick_members",
    "ban_members",
    "manage_nicknames",
    "view_channels",
    "send_messages",
    "attach_files",
    "manage_messages",
    "mention_everyone",
    "connect",
    "speak",
    "mute_members",
    "move_members",
];

pub const DEFAULT_PERMISSIONS: &[&str] = &[
    "view_channels",
    "send_messages",
    "attach_files",
    "create_invite",
    "connect",
    "speak",
];

pub fn is_known_permission(permission: &str) -> bool {
    ALL_PERMISSIONS.contains(&permission)
}

pub fn unknown_permissions(permissions: &[String]) -> Vec<String> {
    permissions
        .iter()
        .filter(|p| !is_known_permission(p))
        .cloned()
        .collect()
}

pub fn unknown_permissions_error(unknown: &[String]) -> (Status, String) {
    let body = serde_json::json!({
        "error": "unknown_permissions",
        "permissions": unknown,
    });
    (Status::BadRequest, body.to_string())
}

pub fn missing_permission_error(permission: &str) -> (Status, String) {
    let body = serde_json::json!({
        "error": "missing_permission",
        "permission": permission,
    });
    (Status::Forbidden, body.to_string())
}

pub fn role_hierarchy_error() -> (Status, String) {
    (Status::Forbidden, r#"{"error":"role_hierarchy"}"#.to_string())
}

pub fn not_member_error() -> (Status, String) {
    (Status::Forbidden, r#"{"error":"not_member"}"#.to_string())
}

#[derive(Debug, Clone)]
pub struct MemberPermissions {
    pub bypass: bool,
    pub permissions: HashSet<String>,
    pub highest_position: i64,
}

impl MemberPermissions {
    pub fn has(&self, permission: &str) -> bool {
        self.bypass || self.permissions.contains(permission)
    }

    // Positions: smaller = higher in the hierarchy. A member may only act on
    // roles strictly below their own highest role.
    pub fn can_act_on_position(&self, position: i32) -> bool {
        self.bypass || (position as i64) > self.highest_position
    }

    pub fn can_grant(&self, added_permissions: &[String]) -> bool {
        self.bypass || added_permissions.iter().all(|p| self.permissions.contains(p))
    }

    pub fn to_list(&self) -> Vec<String> {
        if self.bypass {
            return ALL_PERMISSIONS.iter().map(|p| p.to_string()).collect();
        }
        let mut list: Vec<String> = self.permissions.iter().cloned().collect();
        list.sort();
        list
    }
}

fn full_permission_set() -> HashSet<String> {
    ALL_PERMISSIONS.iter().map(|p| p.to_string()).collect()
}

pub fn highest_role_position(roles: &[Role]) -> i64 {
    roles
        .iter()
        .map(|r| r.position as i64)
        .min()
        .unwrap_or(i64::MAX)
}

async fn fetch_member_roles(
    db: &Surreal<Any>,
    role_ids: Vec<Thing>,
) -> Result<Vec<Role>, (Status, String)> {
    if role_ids.is_empty() {
        return Ok(vec![]);
    }
    db.query("SELECT * FROM $role_ids")
        .bind(("role_ids", role_ids))
        .await
        .map_err(|e| (Status::InternalServerError, e.to_string()))?
        .take(0)
        .map_err(|e| (Status::InternalServerError, e.to_string()))
}

pub async fn get_member_permissions_with_roles(
    db: &Surreal<Any>,
    guild_id: &str,
    user_id: &str,
) -> Result<(MemberPermissions, Vec<Thing>), (Status, String)> {
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
        return Ok((
            MemberPermissions {
                bypass: true,
                permissions: full_permission_set(),
                highest_position: i64::MIN,
            },
            vec![],
        ));
    }

    let memberships: Vec<MemberOf> = db
        .query("SELECT * FROM member_of WHERE `in` = $user_id AND out = $guild_id")
        .bind(("user_id", user_thing))
        .bind(("guild_id", guild_thing))
        .await
        .map_err(|e| (Status::InternalServerError, e.to_string()))?
        .take(0)
        .map_err(|e| (Status::InternalServerError, e.to_string()))?;

    let membership = memberships.into_iter().next().ok_or_else(not_member_error)?;
    let role_ids = membership.roles.clone();

    let roles = fetch_member_roles(db, membership.roles).await?;
    let highest_position = highest_role_position(&roles);

    let mut permissions: HashSet<String> =
        DEFAULT_PERMISSIONS.iter().map(|p| p.to_string()).collect();
    for role in &roles {
        for p in &role.permissions {
            if is_known_permission(p) {
                permissions.insert(p.clone());
            }
        }
    }

    if permissions.contains("administrator") {
        return Ok((
            MemberPermissions {
                bypass: true,
                permissions: full_permission_set(),
                highest_position,
            },
            role_ids,
        ));
    }

    Ok((
        MemberPermissions {
            bypass: false,
            permissions,
            highest_position,
        },
        role_ids,
    ))
}

pub async fn get_member_permissions(
    db: &Surreal<Any>,
    guild_id: &str,
    user_id: &str,
) -> Result<MemberPermissions, (Status, String)> {
    get_member_permissions_with_roles(db, guild_id, user_id)
        .await
        .map(|(perms, _)| perms)
}

// Applies channel permission overwrites on top of guild-level permissions.
// Priority (lowest to highest): base, role denies, role allows, user deny,
// user allow. `administrator`/owner bypass ignores overwrites entirely.
pub fn resolve_channel_overwrites(
    base: MemberPermissions,
    member_role_ids: &[Thing],
    user_id: &str,
    overwrites: &[PermissionOverwrite],
) -> MemberPermissions {
    if base.bypass {
        return base;
    }

    let role_targets: HashSet<String> = member_role_ids.iter().map(|t| t.to_raw()).collect();
    let mut permissions = base.permissions;

    for ow in overwrites.iter().filter(|ow| role_targets.contains(&ow.target)) {
        for p in &ow.deny {
            permissions.remove(p);
        }
    }
    for ow in overwrites.iter().filter(|ow| role_targets.contains(&ow.target)) {
        for p in &ow.allow {
            if is_known_permission(p) {
                permissions.insert(p.clone());
            }
        }
    }
    if let Some(ow) = overwrites.iter().find(|ow| ow.target == user_id) {
        for p in &ow.deny {
            permissions.remove(p);
        }
        for p in &ow.allow {
            if is_known_permission(p) {
                permissions.insert(p.clone());
            }
        }
    }

    MemberPermissions {
        bypass: false,
        permissions,
        highest_position: base.highest_position,
    }
}

pub async fn get_channel_permissions(
    db: &Surreal<Any>,
    channel: &Channel,
    user_id: &str,
) -> Result<MemberPermissions, (Status, String)> {
    let (base, role_ids) =
        get_member_permissions_with_roles(db, &channel.guild.to_raw(), user_id).await?;
    Ok(resolve_channel_overwrites(
        base,
        &role_ids,
        user_id,
        &channel.permission_overwrites,
    ))
}

pub async fn require_permission(
    db: &Surreal<Any>,
    guild_id: &str,
    user_id: &str,
    permission: &str,
) -> Result<MemberPermissions, (Status, String)> {
    let perms = get_member_permissions(db, guild_id, user_id).await?;
    if !perms.has(permission) {
        return Err(missing_permission_error(permission));
    }
    Ok(perms)
}

pub async fn member_highest_position(
    db: &Surreal<Any>,
    guild_thing: &Thing,
    user_thing: &Thing,
) -> Result<i64, (Status, String)> {
    let memberships: Vec<MemberOf> = db
        .query("SELECT * FROM member_of WHERE `in` = $user_id AND out = $guild_id")
        .bind(("user_id", user_thing.clone()))
        .bind(("guild_id", guild_thing.clone()))
        .await
        .map_err(|e| (Status::InternalServerError, e.to_string()))?
        .take(0)
        .map_err(|e| (Status::InternalServerError, e.to_string()))?;

    let membership = memberships
        .into_iter()
        .next()
        .ok_or((Status::NotFound, "User is not a member of this guild".to_string()))?;

    let roles = fetch_member_roles(db, membership.roles).await?;
    Ok(highest_role_position(&roles))
}

// Checks whether `sender_id` may post in guild channel `channel_id`.
// Returns a stable error code on rejection, meant for the WS error event.
pub async fn check_channel_send(
    db: &Surreal<Any>,
    channel_id: &str,
    sender_id: &str,
    has_attachments: bool,
) -> Result<(), String> {
    let channel_thing =
        surrealdb::sql::thing(channel_id).map_err(|_| "invalid_channel".to_string())?;

    let channel: Option<Channel> = db
        .query("SELECT * FROM $channel_id")
        .bind(("channel_id", channel_thing))
        .await
        .map_err(|e| e.to_string())?
        .take(0)
        .map_err(|e| e.to_string())?;

    let channel = channel.ok_or_else(|| "channel_not_found".to_string())?;

    let perms = get_channel_permissions(db, &channel, sender_id)
        .await
        .map_err(|_| "not_member".to_string())?;

    if !perms.has("send_messages") {
        return Err("missing_permission:send_messages".to_string());
    }
    if has_attachments && !perms.has("attach_files") {
        return Err("missing_permission:attach_files".to_string());
    }
    Ok(())
}

// Checks whether `user_id` may join voice channel `channel_id`.
// Returns the channel on success, or a stable error code for the WS error event.
pub async fn check_voice_join(
    db: &Surreal<Any>,
    channel_id: &str,
    user_id: &str,
) -> Result<Channel, String> {
    let channel_thing =
        surrealdb::sql::thing(channel_id).map_err(|_| "invalid_channel".to_string())?;

    let channel: Option<Channel> = db
        .query("SELECT * FROM $channel_id")
        .bind(("channel_id", channel_thing))
        .await
        .map_err(|e| e.to_string())?
        .take(0)
        .map_err(|e| e.to_string())?;

    let channel = channel.ok_or_else(|| "channel_not_found".to_string())?;

    if !matches!(channel.channel_type, ChannelType::Voice) {
        return Err("not_voice_channel".to_string());
    }

    let perms = get_channel_permissions(db, &channel, user_id)
        .await
        .map_err(|_| "not_member".to_string())?;

    if !perms.has("connect") {
        return Err("missing_permission:connect".to_string());
    }

    Ok(channel)
}

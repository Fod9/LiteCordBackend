mod common;

use litecord_backend::guild_channels::{create_channel, delete_channel};
use litecord_backend::guilds::{
    create_guild, create_invite, get_my_membership, join_guild_directly, kick_member,
    list_guild_invites, list_user_guilds, revoke_invite, update_guild,
};
use litecord_backend::messages::assert_channel_access;
use litecord_backend::models::db::{ChannelType, Role};
use litecord_backend::permissions::{
    ALL_PERMISSIONS, DEFAULT_PERMISSIONS, check_channel_send, get_member_permissions,
};
use litecord_backend::roles::{assign_role, create_role, delete_role, update_role};
use rocket::http::Status;
use surrealdb::Surreal;
use surrealdb::engine::any::Any;
use surrealdb::sql::Thing;

async fn setup_guild(db: &Surreal<Any>, tag: &str) -> (Thing, Thing) {
    let owner_id =
        common::create_test_user(db, &format!("owner_{tag}"), &format!("owner_{tag}@test.com"))
            .await;
    let guild = create_guild(db, &owner_id.to_raw(), format!("Guild {tag}"), "".to_string())
        .await
        .unwrap();
    (owner_id, guild.id.unwrap())
}

async fn join_member(db: &Surreal<Any>, guild_id: &Thing, tag: &str) -> Thing {
    let user_id =
        common::create_test_user(db, &format!("member_{tag}"), &format!("member_{tag}@test.com"))
            .await;
    join_guild_directly(db, guild_id, &user_id).await.unwrap();
    user_id
}

async fn give_role(
    db: &Surreal<Any>,
    guild_id: &Thing,
    owner_id: &Thing,
    member_id: &Thing,
    name: &str,
    position: i32,
    permissions: Vec<&str>,
) -> Role {
    let role = create_role(
        db,
        &guild_id.to_raw(),
        &owner_id.to_raw(),
        name.to_string(),
        "#ffffff".to_string(),
        position,
        permissions.into_iter().map(String::from).collect(),
    )
    .await
    .unwrap();
    assign_role(
        db,
        &guild_id.to_raw(),
        &role.id.clone().unwrap().to_raw(),
        &member_id.to_raw(),
        &owner_id.to_raw(),
    )
    .await
    .unwrap();
    role
}

#[tokio::test]
async fn owner_has_all_permissions() {
    let db = common::setup_db().await;
    let (owner_id, guild_id) = setup_guild(&db, "p1").await;

    let perms = get_member_permissions(&db, &guild_id.to_raw(), &owner_id.to_raw())
        .await
        .unwrap();

    assert!(perms.bypass);
    for p in ALL_PERMISSIONS {
        assert!(perms.has(p), "owner should have {p}");
    }
}

#[tokio::test]
async fn plain_member_has_default_permissions_only() {
    let db = common::setup_db().await;
    let (_, guild_id) = setup_guild(&db, "p2").await;
    let member_id = join_member(&db, &guild_id, "p2").await;

    let perms = get_member_permissions(&db, &guild_id.to_raw(), &member_id.to_raw())
        .await
        .unwrap();

    assert!(!perms.bypass);
    for p in DEFAULT_PERMISSIONS {
        assert!(perms.has(p), "member should have default {p}");
    }
    assert!(!perms.has("manage_channels"));
    assert!(!perms.has("kick_members"));
    assert!(!perms.has("manage_roles"));
}

#[tokio::test]
async fn administrator_role_grants_everything() {
    let db = common::setup_db().await;
    let (owner_id, guild_id) = setup_guild(&db, "p3").await;
    let member_id = join_member(&db, &guild_id, "p3").await;
    give_role(&db, &guild_id, &owner_id, &member_id, "Admin", 0, vec!["administrator"]).await;

    let perms = get_member_permissions(&db, &guild_id.to_raw(), &member_id.to_raw())
        .await
        .unwrap();

    assert!(perms.bypass);
    for p in ALL_PERMISSIONS {
        assert!(perms.has(p), "admin should have {p}");
    }
}

#[tokio::test]
async fn non_member_has_no_permissions() {
    let db = common::setup_db().await;
    let (_, guild_id) = setup_guild(&db, "p4").await;
    let outsider = common::create_test_user(&db, "outsider_p4", "outsider_p4@test.com").await;

    let result = get_member_permissions(&db, &guild_id.to_raw(), &outsider.to_raw()).await;
    let (status, body) = result.unwrap_err();
    assert_eq!(status, Status::Forbidden);
    assert!(body.contains("not_member"));
}

#[tokio::test]
async fn create_role_rejects_unknown_permissions() {
    let db = common::setup_db().await;
    let (owner_id, guild_id) = setup_guild(&db, "v1").await;

    let result = create_role(
        &db,
        &guild_id.to_raw(),
        &owner_id.to_raw(),
        "Bad".to_string(),
        "#000000".to_string(),
        1,
        vec!["kick_members".to_string(), "fly_to_the_moon".to_string()],
    )
    .await;

    let (status, body) = result.unwrap_err();
    assert_eq!(status, Status::BadRequest);
    assert!(body.contains("unknown_permissions"));
    assert!(body.contains("fly_to_the_moon"));
    assert!(!body.contains("kick_members"));
}

#[tokio::test]
async fn member_with_manage_roles_can_create_lower_role() {
    let db = common::setup_db().await;
    let (owner_id, guild_id) = setup_guild(&db, "r1").await;
    let mod_id = join_member(&db, &guild_id, "r1").await;
    give_role(&db, &guild_id, &owner_id, &mod_id, "Mod", 1, vec!["manage_roles", "kick_members"]).await;

    let result = create_role(
        &db,
        &guild_id.to_raw(),
        &mod_id.to_raw(),
        "Below".to_string(),
        "#00ff00".to_string(),
        2,
        vec!["kick_members".to_string()],
    )
    .await;

    assert!(result.is_ok(), "mod should create a lower role: {:?}", result.err());
}

#[tokio::test]
async fn member_cannot_create_role_at_or_above_own_position() {
    let db = common::setup_db().await;
    let (owner_id, guild_id) = setup_guild(&db, "r2").await;
    let mod_id = join_member(&db, &guild_id, "r2").await;
    give_role(&db, &guild_id, &owner_id, &mod_id, "Mod", 1, vec!["manage_roles"]).await;

    for position in [0, 1] {
        let result = create_role(
            &db,
            &guild_id.to_raw(),
            &mod_id.to_raw(),
            "TooHigh".to_string(),
            "#ff0000".to_string(),
            position,
            vec![],
        )
        .await;
        let (status, body) = result.unwrap_err();
        assert_eq!(status, Status::Forbidden);
        assert!(body.contains("role_hierarchy"));
    }
}

#[tokio::test]
async fn member_cannot_grant_permission_they_do_not_have() {
    let db = common::setup_db().await;
    let (owner_id, guild_id) = setup_guild(&db, "r3").await;
    let mod_id = join_member(&db, &guild_id, "r3").await;
    give_role(&db, &guild_id, &owner_id, &mod_id, "Mod", 1, vec!["manage_roles"]).await;

    let result = create_role(
        &db,
        &guild_id.to_raw(),
        &mod_id.to_raw(),
        "Sneaky".to_string(),
        "#ff0000".to_string(),
        2,
        vec!["administrator".to_string()],
    )
    .await;

    let (status, body) = result.unwrap_err();
    assert_eq!(status, Status::Forbidden);
    assert!(body.contains("role_hierarchy"));
}

#[tokio::test]
async fn member_without_manage_roles_cannot_create_role() {
    let db = common::setup_db().await;
    let (_, guild_id) = setup_guild(&db, "r4").await;
    let member_id = join_member(&db, &guild_id, "r4").await;

    let result = create_role(
        &db,
        &guild_id.to_raw(),
        &member_id.to_raw(),
        "Nope".to_string(),
        "#ff0000".to_string(),
        2,
        vec![],
    )
    .await;

    let (status, body) = result.unwrap_err();
    assert_eq!(status, Status::Forbidden);
    assert!(body.contains("missing_permission"));
    assert!(body.contains("manage_roles"));
}

#[tokio::test]
async fn update_role_by_owner_updates_fields() {
    let db = common::setup_db().await;
    let (owner_id, guild_id) = setup_guild(&db, "u1").await;
    let role = create_role(&db, &guild_id.to_raw(), &owner_id.to_raw(), "Old".to_string(), "#000000".to_string(), 1, vec![])
        .await
        .unwrap();

    let updated = update_role(
        &db,
        &guild_id.to_raw(),
        &role.id.unwrap().to_raw(),
        &owner_id.to_raw(),
        Some("New".to_string()),
        None,
        None,
        Some(vec!["kick_members".to_string(), "manage_messages".to_string()]),
    )
    .await
    .unwrap();

    assert_eq!(updated.name, "New");
    assert_eq!(updated.color, "#000000");
    assert_eq!(updated.position, 1);
    assert_eq!(updated.permissions, vec!["kick_members", "manage_messages"]);
}

#[tokio::test]
async fn update_role_rejects_unknown_permissions() {
    let db = common::setup_db().await;
    let (owner_id, guild_id) = setup_guild(&db, "u2").await;
    let role = create_role(&db, &guild_id.to_raw(), &owner_id.to_raw(), "Role".to_string(), "#000000".to_string(), 1, vec![])
        .await
        .unwrap();

    let result = update_role(
        &db,
        &guild_id.to_raw(),
        &role.id.unwrap().to_raw(),
        &owner_id.to_raw(),
        None,
        None,
        None,
        Some(vec!["does_not_exist".to_string()]),
    )
    .await;

    let (status, body) = result.unwrap_err();
    assert_eq!(status, Status::BadRequest);
    assert!(body.contains("unknown_permissions"));
}

#[tokio::test]
async fn update_role_from_other_guild_returns_404() {
    let db = common::setup_db().await;
    let (owner_a, guild_a) = setup_guild(&db, "u3a").await;
    let (owner_b, guild_b) = setup_guild(&db, "u3b").await;
    let _ = owner_b;
    let foreign_role = create_role(&db, &guild_b.to_raw(), &owner_b.to_raw(), "Foreign".to_string(), "#000000".to_string(), 1, vec![])
        .await
        .unwrap();

    let result = update_role(
        &db,
        &guild_a.to_raw(),
        &foreign_role.id.unwrap().to_raw(),
        &owner_a.to_raw(),
        Some("Hijack".to_string()),
        None,
        None,
        None,
    )
    .await;

    let (status, _) = result.unwrap_err();
    assert_eq!(status, Status::NotFound);
}

#[tokio::test]
async fn update_role_hierarchy_blocks_editing_higher_role() {
    let db = common::setup_db().await;
    let (owner_id, guild_id) = setup_guild(&db, "u4").await;
    let mod_id = join_member(&db, &guild_id, "u4").await;
    let top_role = create_role(&db, &guild_id.to_raw(), &owner_id.to_raw(), "Top".to_string(), "#000000".to_string(), 0, vec![])
        .await
        .unwrap();
    give_role(&db, &guild_id, &owner_id, &mod_id, "Mod", 1, vec!["manage_roles"]).await;

    let result = update_role(
        &db,
        &guild_id.to_raw(),
        &top_role.id.unwrap().to_raw(),
        &mod_id.to_raw(),
        Some("Pwned".to_string()),
        None,
        None,
        None,
    )
    .await;

    let (status, body) = result.unwrap_err();
    assert_eq!(status, Status::Forbidden);
    assert!(body.contains("role_hierarchy"));
}

#[tokio::test]
async fn update_role_hierarchy_blocks_moving_role_above_self() {
    let db = common::setup_db().await;
    let (owner_id, guild_id) = setup_guild(&db, "u5").await;
    let mod_id = join_member(&db, &guild_id, "u5").await;
    give_role(&db, &guild_id, &owner_id, &mod_id, "Mod", 1, vec!["manage_roles"]).await;
    let low_role = create_role(&db, &guild_id.to_raw(), &owner_id.to_raw(), "Low".to_string(), "#000000".to_string(), 5, vec![])
        .await
        .unwrap();

    let result = update_role(
        &db,
        &guild_id.to_raw(),
        &low_role.id.unwrap().to_raw(),
        &mod_id.to_raw(),
        None,
        None,
        Some(0),
        None,
    )
    .await;

    let (status, body) = result.unwrap_err();
    assert_eq!(status, Status::Forbidden);
    assert!(body.contains("role_hierarchy"));
}

#[tokio::test]
async fn update_role_cannot_grant_unowned_permission() {
    let db = common::setup_db().await;
    let (owner_id, guild_id) = setup_guild(&db, "u6").await;
    let mod_id = join_member(&db, &guild_id, "u6").await;
    give_role(&db, &guild_id, &owner_id, &mod_id, "Mod", 1, vec!["manage_roles"]).await;
    let low_role = create_role(&db, &guild_id.to_raw(), &owner_id.to_raw(), "Low".to_string(), "#000000".to_string(), 5, vec![])
        .await
        .unwrap();

    let result = update_role(
        &db,
        &guild_id.to_raw(),
        &low_role.id.unwrap().to_raw(),
        &mod_id.to_raw(),
        None,
        None,
        None,
        Some(vec!["manage_guild".to_string()]),
    )
    .await;

    let (status, body) = result.unwrap_err();
    assert_eq!(status, Status::Forbidden);
    assert!(body.contains("role_hierarchy"));
}

#[tokio::test]
async fn update_role_purges_unknown_stored_permissions() {
    let db = common::setup_db().await;
    let (owner_id, guild_id) = setup_guild(&db, "u7").await;

    let role: Option<Role> = db
        .query("CREATE role SET guild = $guild, name = 'Legacy', color = '#000000', position = 3, permissions = ['kick_members', 'legacy_perm']")
        .bind(("guild", guild_id.clone()))
        .await
        .unwrap()
        .take(0)
        .unwrap();
    let role_id = role.unwrap().id.unwrap().to_raw();

    let updated = update_role(
        &db,
        &guild_id.to_raw(),
        &role_id,
        &owner_id.to_raw(),
        Some("Renamed".to_string()),
        None,
        None,
        None,
    )
    .await
    .unwrap();

    assert_eq!(updated.permissions, vec!["kick_members"]);
}

#[tokio::test]
async fn delete_role_hierarchy_blocks_deleting_higher_role() {
    let db = common::setup_db().await;
    let (owner_id, guild_id) = setup_guild(&db, "d1").await;
    let mod_id = join_member(&db, &guild_id, "d1").await;
    let top_role = create_role(&db, &guild_id.to_raw(), &owner_id.to_raw(), "Top".to_string(), "#000000".to_string(), 0, vec![])
        .await
        .unwrap();
    give_role(&db, &guild_id, &owner_id, &mod_id, "Mod", 1, vec!["manage_roles"]).await;

    let result = delete_role(&db, &guild_id.to_raw(), &top_role.id.unwrap().to_raw(), &mod_id.to_raw()).await;

    let (status, body) = result.unwrap_err();
    assert_eq!(status, Status::Forbidden);
    assert!(body.contains("role_hierarchy"));
}

#[tokio::test]
async fn assign_role_from_other_guild_returns_404() {
    let db = common::setup_db().await;
    let (owner_a, guild_a) = setup_guild(&db, "a1a").await;
    let (owner_b, guild_b) = setup_guild(&db, "a1b").await;
    let foreign_role = create_role(&db, &guild_b.to_raw(), &owner_b.to_raw(), "Foreign".to_string(), "#000000".to_string(), 1, vec!["administrator".to_string()])
        .await
        .unwrap();

    let result = assign_role(
        &db,
        &guild_a.to_raw(),
        &foreign_role.id.unwrap().to_raw(),
        &owner_a.to_raw(),
        &owner_a.to_raw(),
    )
    .await;

    let (status, _) = result.unwrap_err();
    assert_eq!(status, Status::NotFound);
}

#[tokio::test]
async fn assign_role_hierarchy_blocks_assigning_higher_role() {
    let db = common::setup_db().await;
    let (owner_id, guild_id) = setup_guild(&db, "a2").await;
    let mod_id = join_member(&db, &guild_id, "a2").await;
    let target_id = join_member(&db, &guild_id, "a2t").await;
    let top_role = create_role(&db, &guild_id.to_raw(), &owner_id.to_raw(), "Top".to_string(), "#000000".to_string(), 0, vec![])
        .await
        .unwrap();
    give_role(&db, &guild_id, &owner_id, &mod_id, "Mod", 1, vec!["manage_roles"]).await;

    let result = assign_role(
        &db,
        &guild_id.to_raw(),
        &top_role.id.unwrap().to_raw(),
        &target_id.to_raw(),
        &mod_id.to_raw(),
    )
    .await;

    let (status, body) = result.unwrap_err();
    assert_eq!(status, Status::Forbidden);
    assert!(body.contains("role_hierarchy"));
}

#[tokio::test]
async fn kick_requires_kick_members_permission() {
    let db = common::setup_db().await;
    let (_, guild_id) = setup_guild(&db, "k1").await;
    let member_id = join_member(&db, &guild_id, "k1").await;
    let target_id = join_member(&db, &guild_id, "k1t").await;

    let result = kick_member(&db, &guild_id.to_raw(), &target_id.to_raw(), &member_id.to_raw()).await;

    let (status, body) = result.unwrap_err();
    assert_eq!(status, Status::Forbidden);
    assert!(body.contains("missing_permission"));
    assert!(body.contains("kick_members"));
}

#[tokio::test]
async fn moderator_can_kick_plain_member() {
    let db = common::setup_db().await;
    let (owner_id, guild_id) = setup_guild(&db, "k2").await;
    let mod_id = join_member(&db, &guild_id, "k2").await;
    let target_id = join_member(&db, &guild_id, "k2t").await;
    give_role(&db, &guild_id, &owner_id, &mod_id, "Mod", 1, vec!["kick_members"]).await;

    kick_member(&db, &guild_id.to_raw(), &target_id.to_raw(), &mod_id.to_raw())
        .await
        .expect("moderator should kick a plain member");

    let guilds = list_user_guilds(&db, &target_id.to_raw()).await.unwrap();
    assert!(guilds.is_empty());
}

#[tokio::test]
async fn moderator_cannot_kick_equal_or_higher_member() {
    let db = common::setup_db().await;
    let (owner_id, guild_id) = setup_guild(&db, "k3").await;
    let mod_id = join_member(&db, &guild_id, "k3").await;
    let peer_id = join_member(&db, &guild_id, "k3p").await;
    give_role(&db, &guild_id, &owner_id, &mod_id, "ModA", 1, vec!["kick_members"]).await;
    give_role(&db, &guild_id, &owner_id, &peer_id, "ModB", 1, vec![]).await;

    let result = kick_member(&db, &guild_id.to_raw(), &peer_id.to_raw(), &mod_id.to_raw()).await;

    let (status, body) = result.unwrap_err();
    assert_eq!(status, Status::Forbidden);
    assert!(body.contains("role_hierarchy"));
}

#[tokio::test]
async fn moderator_cannot_kick_owner() {
    let db = common::setup_db().await;
    let (owner_id, guild_id) = setup_guild(&db, "k4").await;
    let mod_id = join_member(&db, &guild_id, "k4").await;
    give_role(&db, &guild_id, &owner_id, &mod_id, "Mod", 1, vec!["kick_members"]).await;

    let result = kick_member(&db, &guild_id.to_raw(), &owner_id.to_raw(), &mod_id.to_raw()).await;

    let (status, _) = result.unwrap_err();
    assert_eq!(status, Status::BadRequest);
}

#[tokio::test]
async fn plain_member_can_create_invite() {
    let db = common::setup_db().await;
    let (_, guild_id) = setup_guild(&db, "i1").await;
    let member_id = join_member(&db, &guild_id, "i1").await;

    let invite = create_invite(&db, &guild_id.to_raw(), &member_id.to_raw()).await;
    assert!(invite.is_ok(), "default permissions include create_invite: {:?}", invite.err());
}

#[tokio::test]
async fn plain_member_cannot_list_or_revoke_invites() {
    let db = common::setup_db().await;
    let (owner_id, guild_id) = setup_guild(&db, "i2").await;
    let member_id = join_member(&db, &guild_id, "i2").await;
    let invite = create_invite(&db, &guild_id.to_raw(), &owner_id.to_raw()).await.unwrap();

    let (status, body) = list_guild_invites(&db, &guild_id.to_raw(), &member_id.to_raw())
        .await
        .unwrap_err();
    assert_eq!(status, Status::Forbidden);
    assert!(body.contains("manage_invites"));

    let (status, _) = revoke_invite(&db, &guild_id.to_raw(), &invite.id.unwrap().to_raw(), &member_id.to_raw())
        .await
        .unwrap_err();
    assert_eq!(status, Status::Forbidden);
}

#[tokio::test]
async fn member_with_manage_invites_can_list_and_revoke() {
    let db = common::setup_db().await;
    let (owner_id, guild_id) = setup_guild(&db, "i3").await;
    let member_id = join_member(&db, &guild_id, "i3").await;
    give_role(&db, &guild_id, &owner_id, &member_id, "InviteMgr", 1, vec!["manage_invites"]).await;
    let invite = create_invite(&db, &guild_id.to_raw(), &owner_id.to_raw()).await.unwrap();

    let invites = list_guild_invites(&db, &guild_id.to_raw(), &member_id.to_raw())
        .await
        .expect("manage_invites should allow listing");
    assert_eq!(invites.len(), 1);

    revoke_invite(&db, &guild_id.to_raw(), &invite.id.unwrap().to_raw(), &member_id.to_raw())
        .await
        .expect("manage_invites should allow revoking");
}

#[tokio::test]
async fn plain_member_cannot_create_channel() {
    let db = common::setup_db().await;
    let (_, guild_id) = setup_guild(&db, "c1").await;
    let member_id = join_member(&db, &guild_id, "c1").await;

    let result = create_channel(&db, &guild_id.to_raw(), &member_id.to_raw(), "nope".to_string(), ChannelType::Text, None).await;

    let (status, body) = result.unwrap_err();
    assert_eq!(status, Status::Forbidden);
    assert!(body.contains("manage_channels"));
}

#[tokio::test]
async fn member_with_manage_channels_can_create_and_delete() {
    let db = common::setup_db().await;
    let (owner_id, guild_id) = setup_guild(&db, "c2").await;
    let builder_id = join_member(&db, &guild_id, "c2").await;
    give_role(&db, &guild_id, &owner_id, &builder_id, "Builder", 1, vec!["manage_channels"]).await;

    let channel = create_channel(&db, &guild_id.to_raw(), &builder_id.to_raw(), "built".to_string(), ChannelType::Text, None)
        .await
        .expect("manage_channels should allow creating");

    delete_channel(&db, &guild_id.to_raw(), &channel.id.unwrap().to_raw(), &builder_id.to_raw())
        .await
        .expect("manage_channels should allow deleting");
}

#[tokio::test]
async fn delete_channel_from_other_guild_returns_404() {
    let db = common::setup_db().await;
    let (owner_a, guild_a) = setup_guild(&db, "c3a").await;
    let (owner_b, guild_b) = setup_guild(&db, "c3b").await;
    let channel = create_channel(&db, &guild_b.to_raw(), &owner_b.to_raw(), "other".to_string(), ChannelType::Text, None)
        .await
        .unwrap();

    let result = delete_channel(&db, &guild_a.to_raw(), &channel.id.unwrap().to_raw(), &owner_a.to_raw()).await;

    let (status, _) = result.unwrap_err();
    assert_eq!(status, Status::NotFound);
}

#[tokio::test]
async fn update_guild_requires_manage_guild() {
    let db = common::setup_db().await;
    let (owner_id, guild_id) = setup_guild(&db, "g1").await;
    let member_id = join_member(&db, &guild_id, "g1").await;

    let (status, body) = update_guild(&db, &guild_id.to_raw(), &member_id.to_raw(), Some("Hack".to_string()), None)
        .await
        .unwrap_err();
    assert_eq!(status, Status::Forbidden);
    assert!(body.contains("manage_guild"));

    give_role(&db, &guild_id, &owner_id, &member_id, "Manager", 1, vec!["manage_guild"]).await;

    let updated = update_guild(&db, &guild_id.to_raw(), &member_id.to_raw(), Some("Renamed".to_string()), None)
        .await
        .expect("manage_guild should allow updating");
    assert_eq!(updated.name, "Renamed");
}

#[tokio::test]
async fn channel_messages_access_is_restricted_to_members() {
    let db = common::setup_db().await;
    let (owner_id, guild_id) = setup_guild(&db, "m1").await;
    let member_id = join_member(&db, &guild_id, "m1").await;
    let outsider = common::create_test_user(&db, "outsider_m1", "outsider_m1@test.com").await;
    let channel = create_channel(&db, &guild_id.to_raw(), &owner_id.to_raw(), "general".to_string(), ChannelType::Text, None)
        .await
        .unwrap();
    let channel_id = channel.id.unwrap().to_raw();

    assert_channel_access(&db, &channel_id, &member_id.to_raw())
        .await
        .expect("member should read channel history");

    let (status, _) = assert_channel_access(&db, &channel_id, &outsider.to_raw())
        .await
        .unwrap_err();
    assert_eq!(status, Status::Forbidden);
}

#[tokio::test]
async fn dm_messages_access_is_restricted_to_recipients() {
    let db = common::setup_db().await;
    let user_a = common::create_test_user(&db, "dm_a", "dm_a@test.com").await;
    let stranger = common::create_test_user(&db, "dm_s", "dm_s@test.com").await;
    let dm_id = common::create_test_dm_channel(&db, &user_a).await;

    assert_channel_access(&db, &dm_id, &user_a.to_raw())
        .await
        .expect("recipient should read DM history");

    let (status, _) = assert_channel_access(&db, &dm_id, &stranger.to_raw())
        .await
        .unwrap_err();
    assert_eq!(status, Status::Forbidden);
}

#[tokio::test]
async fn ws_send_rejects_non_members() {
    let db = common::setup_db().await;
    let (owner_id, guild_id) = setup_guild(&db, "w1").await;
    let member_id = join_member(&db, &guild_id, "w1").await;
    let outsider = common::create_test_user(&db, "outsider_w1", "outsider_w1@test.com").await;
    let channel = create_channel(&db, &guild_id.to_raw(), &owner_id.to_raw(), "general".to_string(), ChannelType::Text, None)
        .await
        .unwrap();
    let channel_id = channel.id.unwrap().to_raw();

    check_channel_send(&db, &channel_id, &member_id.to_raw(), true)
        .await
        .expect("member with default permissions should send messages with attachments");

    let err = check_channel_send(&db, &channel_id, &outsider.to_raw(), false)
        .await
        .unwrap_err();
    assert_eq!(err, "not_member");
}

#[tokio::test]
async fn members_me_returns_member_and_computed_permissions() {
    let db = common::setup_db().await;
    let (owner_id, guild_id) = setup_guild(&db, "me1").await;
    let member_id = join_member(&db, &guild_id, "me1").await;

    let (owner_profile, owner_perms) = get_my_membership(&db, &guild_id.to_raw(), &owner_id.to_raw())
        .await
        .unwrap();
    assert_eq!(owner_profile.user.id.to_raw(), owner_id.to_raw());
    assert_eq!(owner_perms.len(), ALL_PERMISSIONS.len());

    let (member_profile, member_perms) = get_my_membership(&db, &guild_id.to_raw(), &member_id.to_raw())
        .await
        .unwrap();
    assert_eq!(member_profile.user.id.to_raw(), member_id.to_raw());
    let mut expected: Vec<String> = DEFAULT_PERMISSIONS.iter().map(|p| p.to_string()).collect();
    expected.sort();
    assert_eq!(member_perms, expected);

    let outsider = common::create_test_user(&db, "outsider_me1", "outsider_me1@test.com").await;
    let (status, _) = get_my_membership(&db, &guild_id.to_raw(), &outsider.to_raw())
        .await
        .unwrap_err();
    assert_eq!(status, Status::Forbidden);
}

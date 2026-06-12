mod common;

use litecord_backend::guild_channels::{
    create_channel, list_guild_channels, update_channel_permissions,
};
use litecord_backend::guilds::{create_guild, join_guild_directly};
use litecord_backend::messages::assert_channel_access;
use litecord_backend::models::db::{ChannelType, PermissionOverwrite, Role};
use litecord_backend::permissions::check_channel_send;
use litecord_backend::roles::{assign_role, create_role};
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

async fn create_text_channel(db: &Surreal<Any>, guild_id: &Thing, owner_id: &Thing, name: &str) -> String {
    create_channel(db, &guild_id.to_raw(), &owner_id.to_raw(), name.to_string(), ChannelType::Text, None)
        .await
        .unwrap()
        .id
        .unwrap()
        .to_raw()
}

fn overwrite(target: &str, allow: Vec<&str>, deny: Vec<&str>) -> PermissionOverwrite {
    PermissionOverwrite {
        target: target.to_string(),
        allow: allow.into_iter().map(String::from).collect(),
        deny: deny.into_iter().map(String::from).collect(),
    }
}

#[tokio::test]
async fn update_permissions_requires_manage_channels() {
    let db = common::setup_db().await;
    let (owner_id, guild_id) = setup_guild(&db, "up1").await;
    let member_id = join_member(&db, &guild_id, "up1").await;
    let channel_id = create_text_channel(&db, &guild_id, &owner_id, "general").await;

    let result = update_channel_permissions(&db, &guild_id.to_raw(), &channel_id, &member_id.to_raw(), vec![]).await;

    let (status, body) = result.unwrap_err();
    assert_eq!(status, Status::Forbidden);
    assert!(body.contains("missing_permission"));
    assert!(body.contains("manage_channels"));
}

#[tokio::test]
async fn update_permissions_rejects_unknown_permission() {
    let db = common::setup_db().await;
    let (owner_id, guild_id) = setup_guild(&db, "up2").await;
    let channel_id = create_text_channel(&db, &guild_id, &owner_id, "general").await;

    let result = update_channel_permissions(
        &db,
        &guild_id.to_raw(),
        &channel_id,
        &owner_id.to_raw(),
        vec![overwrite(&owner_id.to_raw(), vec!["fly_to_the_moon"], vec![])],
    )
    .await;

    let (status, body) = result.unwrap_err();
    assert_eq!(status, Status::BadRequest);
    assert!(body.contains("unknown_permissions"));
    assert!(body.contains("fly_to_the_moon"));
}

#[tokio::test]
async fn update_permissions_rejects_invalid_target() {
    let db = common::setup_db().await;
    let (owner_id, guild_id) = setup_guild(&db, "up3").await;
    let channel_id = create_text_channel(&db, &guild_id, &owner_id, "general").await;

    let result = update_channel_permissions(
        &db,
        &guild_id.to_raw(),
        &channel_id,
        &owner_id.to_raw(),
        vec![overwrite("guild:nope", vec![], vec!["send_messages"])],
    )
    .await;

    let (status, body) = result.unwrap_err();
    assert_eq!(status, Status::BadRequest);
    assert!(body.contains("invalid_target"));
}

#[tokio::test]
async fn update_permissions_on_foreign_channel_returns_404() {
    let db = common::setup_db().await;
    let (owner_a, guild_a) = setup_guild(&db, "up4a").await;
    let (owner_b, guild_b) = setup_guild(&db, "up4b").await;
    let foreign_channel = create_text_channel(&db, &guild_b, &owner_b, "other").await;

    let result = update_channel_permissions(&db, &guild_a.to_raw(), &foreign_channel, &owner_a.to_raw(), vec![]).await;

    let (status, _) = result.unwrap_err();
    assert_eq!(status, Status::NotFound);
}

#[tokio::test]
async fn update_permissions_persists_and_returns_overwrites() {
    let db = common::setup_db().await;
    let (owner_id, guild_id) = setup_guild(&db, "up5").await;
    let member_id = join_member(&db, &guild_id, "up5").await;
    let channel_id = create_text_channel(&db, &guild_id, &owner_id, "secret").await;

    let updated = update_channel_permissions(
        &db,
        &guild_id.to_raw(),
        &channel_id,
        &owner_id.to_raw(),
        vec![overwrite(&member_id.to_raw(), vec![], vec!["view_channels"])],
    )
    .await
    .unwrap();

    assert_eq!(updated.permission_overwrites.len(), 1);
    assert_eq!(updated.permission_overwrites[0].target, member_id.to_raw());
    assert_eq!(updated.permission_overwrites[0].deny, vec!["view_channels"]);

    let replaced = update_channel_permissions(&db, &guild_id.to_raw(), &channel_id, &owner_id.to_raw(), vec![])
        .await
        .unwrap();
    assert!(replaced.permission_overwrites.is_empty());
}

#[tokio::test]
async fn channel_without_overwrites_keeps_default_behavior() {
    let db = common::setup_db().await;
    let (owner_id, guild_id) = setup_guild(&db, "ow0").await;
    let member_id = join_member(&db, &guild_id, "ow0").await;
    let channel_id = create_text_channel(&db, &guild_id, &owner_id, "general").await;

    check_channel_send(&db, &channel_id, &member_id.to_raw(), false)
        .await
        .expect("member should send without overwrites");
}

#[tokio::test]
async fn role_deny_blocks_send_messages() {
    let db = common::setup_db().await;
    let (owner_id, guild_id) = setup_guild(&db, "ow1").await;
    let member_id = join_member(&db, &guild_id, "ow1").await;
    let role = give_role(&db, &guild_id, &owner_id, &member_id, "Muted", 5, vec![]).await;
    let channel_id = create_text_channel(&db, &guild_id, &owner_id, "general").await;

    update_channel_permissions(
        &db,
        &guild_id.to_raw(),
        &channel_id,
        &owner_id.to_raw(),
        vec![overwrite(&role.id.unwrap().to_raw(), vec![], vec!["send_messages"])],
    )
    .await
    .unwrap();

    let err = check_channel_send(&db, &channel_id, &member_id.to_raw(), false)
        .await
        .unwrap_err();
    assert_eq!(err, "missing_permission:send_messages");
}

#[tokio::test]
async fn role_allow_overrides_role_deny() {
    let db = common::setup_db().await;
    let (owner_id, guild_id) = setup_guild(&db, "ow2").await;
    let member_id = join_member(&db, &guild_id, "ow2").await;
    let muted = give_role(&db, &guild_id, &owner_id, &member_id, "Muted", 5, vec![]).await;
    let speaker = give_role(&db, &guild_id, &owner_id, &member_id, "Speaker", 4, vec![]).await;
    let channel_id = create_text_channel(&db, &guild_id, &owner_id, "general").await;

    update_channel_permissions(
        &db,
        &guild_id.to_raw(),
        &channel_id,
        &owner_id.to_raw(),
        vec![
            overwrite(&muted.id.unwrap().to_raw(), vec![], vec!["send_messages"]),
            overwrite(&speaker.id.unwrap().to_raw(), vec!["send_messages"], vec![]),
        ],
    )
    .await
    .unwrap();

    check_channel_send(&db, &channel_id, &member_id.to_raw(), false)
        .await
        .expect("role allow should override role deny");
}

#[tokio::test]
async fn user_deny_overrides_role_allow() {
    let db = common::setup_db().await;
    let (owner_id, guild_id) = setup_guild(&db, "ow3").await;
    let member_id = join_member(&db, &guild_id, "ow3").await;
    let role = give_role(&db, &guild_id, &owner_id, &member_id, "Speaker", 4, vec![]).await;
    let channel_id = create_text_channel(&db, &guild_id, &owner_id, "general").await;

    update_channel_permissions(
        &db,
        &guild_id.to_raw(),
        &channel_id,
        &owner_id.to_raw(),
        vec![
            overwrite(&role.id.unwrap().to_raw(), vec!["send_messages"], vec![]),
            overwrite(&member_id.to_raw(), vec![], vec!["send_messages"]),
        ],
    )
    .await
    .unwrap();

    let err = check_channel_send(&db, &channel_id, &member_id.to_raw(), false)
        .await
        .unwrap_err();
    assert_eq!(err, "missing_permission:send_messages");
}

#[tokio::test]
async fn user_allow_overrides_role_deny() {
    let db = common::setup_db().await;
    let (owner_id, guild_id) = setup_guild(&db, "ow4").await;
    let member_id = join_member(&db, &guild_id, "ow4").await;
    let role = give_role(&db, &guild_id, &owner_id, &member_id, "Muted", 5, vec![]).await;
    let channel_id = create_text_channel(&db, &guild_id, &owner_id, "general").await;

    update_channel_permissions(
        &db,
        &guild_id.to_raw(),
        &channel_id,
        &owner_id.to_raw(),
        vec![
            overwrite(&role.id.unwrap().to_raw(), vec![], vec!["send_messages"]),
            overwrite(&member_id.to_raw(), vec!["send_messages"], vec![]),
        ],
    )
    .await
    .unwrap();

    check_channel_send(&db, &channel_id, &member_id.to_raw(), false)
        .await
        .expect("user allow should override role deny");
}

#[tokio::test]
async fn administrator_bypasses_overwrites() {
    let db = common::setup_db().await;
    let (owner_id, guild_id) = setup_guild(&db, "ow5").await;
    let admin_id = join_member(&db, &guild_id, "ow5").await;
    give_role(&db, &guild_id, &owner_id, &admin_id, "Admin", 0, vec!["administrator"]).await;
    let channel_id = create_text_channel(&db, &guild_id, &owner_id, "general").await;

    update_channel_permissions(
        &db,
        &guild_id.to_raw(),
        &channel_id,
        &owner_id.to_raw(),
        vec![overwrite(&admin_id.to_raw(), vec![], vec!["send_messages", "view_channels"])],
    )
    .await
    .unwrap();

    check_channel_send(&db, &channel_id, &admin_id.to_raw(), false)
        .await
        .expect("administrator bypasses overwrites");
}

#[tokio::test]
async fn view_channels_deny_hides_channel_from_list() {
    let db = common::setup_db().await;
    let (owner_id, guild_id) = setup_guild(&db, "ls1").await;
    let member_id = join_member(&db, &guild_id, "ls1").await;
    let visible_id = create_text_channel(&db, &guild_id, &owner_id, "general").await;
    let hidden_id = create_text_channel(&db, &guild_id, &owner_id, "secret").await;

    update_channel_permissions(
        &db,
        &guild_id.to_raw(),
        &hidden_id,
        &owner_id.to_raw(),
        vec![overwrite(&member_id.to_raw(), vec![], vec!["view_channels"])],
    )
    .await
    .unwrap();

    let member_channels = list_guild_channels(&db, &guild_id.to_raw(), &member_id.to_raw())
        .await
        .unwrap();
    let member_ids: Vec<String> = member_channels.iter().map(|c| c.id.as_ref().unwrap().to_raw()).collect();
    assert!(member_ids.contains(&visible_id));
    assert!(!member_ids.contains(&hidden_id));

    let owner_channels = list_guild_channels(&db, &guild_id.to_raw(), &owner_id.to_raw())
        .await
        .unwrap();
    assert_eq!(owner_channels.len(), 2);
}

#[tokio::test]
async fn view_channels_deny_blocks_message_history() {
    let db = common::setup_db().await;
    let (owner_id, guild_id) = setup_guild(&db, "ls2").await;
    let member_id = join_member(&db, &guild_id, "ls2").await;
    let channel_id = create_text_channel(&db, &guild_id, &owner_id, "secret").await;

    update_channel_permissions(
        &db,
        &guild_id.to_raw(),
        &channel_id,
        &owner_id.to_raw(),
        vec![overwrite(&member_id.to_raw(), vec![], vec!["view_channels"])],
    )
    .await
    .unwrap();

    let (status, body) = assert_channel_access(&db, &channel_id, &member_id.to_raw())
        .await
        .unwrap_err();
    assert_eq!(status, Status::Forbidden);
    assert!(body.contains("view_channels"));
}

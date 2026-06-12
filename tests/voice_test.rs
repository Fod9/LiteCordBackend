mod common;

use litecord_backend::chat::hub::ChatHub;
use litecord_backend::guild_channels::{create_channel, update_channel_permissions};
use litecord_backend::guilds::{create_guild, join_guild_directly};
use litecord_backend::models::db::{ChannelType, PermissionOverwrite};
use litecord_backend::permissions::check_voice_join;
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

async fn create_voice_channel(db: &Surreal<Any>, guild_id: &Thing, owner_id: &Thing, name: &str) -> String {
    create_channel(db, &guild_id.to_raw(), &owner_id.to_raw(), name.to_string(), ChannelType::Voice, None)
        .await
        .unwrap()
        .id
        .unwrap()
        .to_raw()
}

#[tokio::test]
async fn member_can_join_voice_channel() {
    let db = common::setup_db().await;
    let (owner_id, guild_id) = setup_guild(&db, "vj1").await;
    let member_id = join_member(&db, &guild_id, "vj1").await;
    let channel_id = create_voice_channel(&db, &guild_id, &owner_id, "vocal").await;

    let channel = check_voice_join(&db, &channel_id, &member_id.to_raw())
        .await
        .expect("member with default permissions should join voice");
    assert_eq!(channel.guild.to_raw(), guild_id.to_raw());
}

#[tokio::test]
async fn voice_join_rejects_text_channel() {
    let db = common::setup_db().await;
    let (owner_id, guild_id) = setup_guild(&db, "vj2").await;
    let channel = create_channel(&db, &guild_id.to_raw(), &owner_id.to_raw(), "texte".to_string(), ChannelType::Text, None)
        .await
        .unwrap();

    let err = check_voice_join(&db, &channel.id.unwrap().to_raw(), &owner_id.to_raw())
        .await
        .unwrap_err();
    assert_eq!(err, "not_voice_channel");
}

#[tokio::test]
async fn voice_join_rejects_unknown_channel() {
    let db = common::setup_db().await;
    let (owner_id, _) = setup_guild(&db, "vj3").await;

    let err = check_voice_join(&db, "channel:doesnotexist", &owner_id.to_raw())
        .await
        .unwrap_err();
    assert_eq!(err, "channel_not_found");
}

#[tokio::test]
async fn voice_join_rejects_non_member() {
    let db = common::setup_db().await;
    let (owner_id, guild_id) = setup_guild(&db, "vj4").await;
    let outsider = common::create_test_user(&db, "outsider_vj4", "outsider_vj4@test.com").await;
    let channel_id = create_voice_channel(&db, &guild_id, &owner_id, "vocal").await;

    let err = check_voice_join(&db, &channel_id, &outsider.to_raw())
        .await
        .unwrap_err();
    assert_eq!(err, "not_member");
}

#[tokio::test]
async fn voice_join_rejects_member_with_connect_denied() {
    let db = common::setup_db().await;
    let (owner_id, guild_id) = setup_guild(&db, "vj5").await;
    let member_id = join_member(&db, &guild_id, "vj5").await;
    let channel_id = create_voice_channel(&db, &guild_id, &owner_id, "vocal").await;

    update_channel_permissions(
        &db,
        &guild_id.to_raw(),
        &channel_id,
        &owner_id.to_raw(),
        vec![PermissionOverwrite {
            target: member_id.to_raw(),
            allow: vec![],
            deny: vec!["connect".to_string()],
        }],
    )
    .await
    .unwrap();

    let err = check_voice_join(&db, &channel_id, &member_id.to_raw())
        .await
        .unwrap_err();
    assert_eq!(err, "missing_permission:connect");

    check_voice_join(&db, &channel_id, &owner_id.to_raw())
        .await
        .expect("owner bypasses overwrites");
}

#[tokio::test]
async fn hub_voice_join_registers_state() {
    let db = common::setup_db().await;
    let hub = ChatHub::new();
    let (owner_id, guild_id) = setup_guild(&db, "h1").await;
    let member_id = join_member(&db, &guild_id, "h1").await;
    let channel_id = create_voice_channel(&db, &guild_id, &owner_id, "vocal").await;

    hub.voice_join(&db, &member_id.to_raw(), &channel_id).await.unwrap();

    let states = hub.voice_states.read().await;
    let state = states.get(&member_id.to_raw()).expect("state registered");
    assert_eq!(state.channel_id, channel_id);
    assert_eq!(state.guild_id, guild_id.to_raw());
}

#[tokio::test]
async fn hub_voice_join_switches_channel_in_same_guild() {
    let db = common::setup_db().await;
    let hub = ChatHub::new();
    let (owner_id, guild_id) = setup_guild(&db, "h2").await;
    let channel_a = create_voice_channel(&db, &guild_id, &owner_id, "vocal-a").await;
    let channel_b = create_voice_channel(&db, &guild_id, &owner_id, "vocal-b").await;

    hub.voice_join(&db, &owner_id.to_raw(), &channel_a).await.unwrap();
    hub.voice_join(&db, &owner_id.to_raw(), &channel_b).await.unwrap();

    let states = hub.voice_states.read().await;
    assert_eq!(states.len(), 1);
    assert_eq!(states.get(&owner_id.to_raw()).unwrap().channel_id, channel_b);
}

#[tokio::test]
async fn hub_voice_join_rejected_does_not_register_state() {
    let db = common::setup_db().await;
    let hub = ChatHub::new();
    let (owner_id, guild_id) = setup_guild(&db, "h3").await;
    let outsider = common::create_test_user(&db, "outsider_h3", "outsider_h3@test.com").await;
    let channel_id = create_voice_channel(&db, &guild_id, &owner_id, "vocal").await;

    let err = hub.voice_join(&db, &outsider.to_raw(), &channel_id).await.unwrap_err();
    assert_eq!(err, "not_member");
    assert!(hub.voice_states.read().await.is_empty());
}

#[tokio::test]
async fn hub_voice_leave_removes_state_and_is_noop_when_absent() {
    let db = common::setup_db().await;
    let hub = ChatHub::new();
    let (owner_id, guild_id) = setup_guild(&db, "h4").await;
    let channel_id = create_voice_channel(&db, &guild_id, &owner_id, "vocal").await;

    hub.voice_leave(&db, &owner_id.to_raw()).await;

    hub.voice_join(&db, &owner_id.to_raw(), &channel_id).await.unwrap();
    hub.voice_leave(&db, &owner_id.to_raw()).await;
    assert!(hub.voice_states.read().await.is_empty());
}

#[tokio::test]
async fn hub_voice_leave_guild_only_affects_matching_guild() {
    let db = common::setup_db().await;
    let hub = ChatHub::new();
    let (owner_id, guild_id) = setup_guild(&db, "h5").await;
    let (_, other_guild) = setup_guild(&db, "h5b").await;
    let channel_id = create_voice_channel(&db, &guild_id, &owner_id, "vocal").await;

    hub.voice_join(&db, &owner_id.to_raw(), &channel_id).await.unwrap();

    hub.voice_leave_guild(&db, &owner_id.to_raw(), &other_guild.to_raw()).await;
    assert_eq!(hub.voice_states.read().await.len(), 1);

    hub.voice_leave_guild(&db, &owner_id.to_raw(), &guild_id.to_raw()).await;
    assert!(hub.voice_states.read().await.is_empty());
}

#[tokio::test]
async fn hub_clear_channel_voice_states_removes_all_occupants() {
    let db = common::setup_db().await;
    let hub = ChatHub::new();
    let (owner_id, guild_id) = setup_guild(&db, "h6").await;
    let member_id = join_member(&db, &guild_id, "h6").await;
    let channel_a = create_voice_channel(&db, &guild_id, &owner_id, "vocal-a").await;
    let channel_b = create_voice_channel(&db, &guild_id, &owner_id, "vocal-b").await;

    hub.voice_join(&db, &owner_id.to_raw(), &channel_a).await.unwrap();
    hub.voice_join(&db, &member_id.to_raw(), &channel_b).await.unwrap();

    hub.clear_channel_voice_states(&db, &guild_id.to_raw(), &channel_a).await;

    let states = hub.voice_states.read().await;
    assert!(!states.contains_key(&owner_id.to_raw()));
    assert!(states.contains_key(&member_id.to_raw()));
}

#[tokio::test]
async fn relay_allowed_between_friends() {
    let db = common::setup_db().await;
    let user_a = common::create_test_user(&db, "relay_a", "relay_a@test.com").await;
    let user_b = common::create_test_user(&db, "relay_b", "relay_b@test.com").await;
    common::create_accepted_friendship(&db, &user_a, &user_b).await;

    assert!(ChatHub::can_relay(&db, &user_a.to_raw(), &user_b.to_raw()).await);
}

#[tokio::test]
async fn relay_allowed_between_guild_members_who_are_not_friends() {
    let db = common::setup_db().await;
    let (owner_id, guild_id) = setup_guild(&db, "r1").await;
    let member_a = join_member(&db, &guild_id, "r1a").await;
    let member_b = join_member(&db, &guild_id, "r1b").await;

    assert!(ChatHub::can_relay(&db, &member_a.to_raw(), &member_b.to_raw()).await);
    assert!(ChatHub::can_relay(&db, &member_a.to_raw(), &owner_id.to_raw()).await);
}

#[tokio::test]
async fn relay_rejected_between_strangers() {
    let db = common::setup_db().await;
    let (_, guild_a) = setup_guild(&db, "r2a").await;
    let (_, guild_b) = setup_guild(&db, "r2b").await;
    let member_a = join_member(&db, &guild_a, "r2a").await;
    let member_b = join_member(&db, &guild_b, "r2b").await;

    assert!(!ChatHub::can_relay(&db, &member_a.to_raw(), &member_b.to_raw()).await);
}

#[tokio::test]
async fn hub_voice_states_for_user_is_scoped_to_their_guilds() {
    let db = common::setup_db().await;
    let hub = ChatHub::new();
    let (owner_a, guild_a) = setup_guild(&db, "h7a").await;
    let (owner_b, guild_b) = setup_guild(&db, "h7b").await;
    let member_id = join_member(&db, &guild_a, "h7").await;
    let channel_a = create_voice_channel(&db, &guild_a, &owner_a, "vocal-a").await;
    let channel_b = create_voice_channel(&db, &guild_b, &owner_b, "vocal-b").await;

    hub.voice_join(&db, &owner_a.to_raw(), &channel_a).await.unwrap();
    hub.voice_join(&db, &owner_b.to_raw(), &channel_b).await.unwrap();

    let states = hub.voice_states_for_user(&db, &member_id.to_raw()).await;
    assert_eq!(states.len(), 1);
    assert_eq!(states[0]["guild_id"], guild_a.to_raw());
    assert_eq!(states[0]["channel_id"], channel_a);
    assert_eq!(states[0]["user"]["id"], owner_a.to_raw());
}

mod common;

use litecord_backend::guild_channels::{create_channel, delete_channel, list_guild_channels};
use litecord_backend::guilds::create_guild;
use litecord_backend::models::db::ChannelType;

async fn setup_guild(
    db: &surrealdb::Surreal<surrealdb::engine::any::Any>,
    owner_name: &str,
    owner_email: &str,
) -> (surrealdb::sql::Thing, String) {
    let owner_id = common::create_test_user(db, owner_name, owner_email).await;
    let guild = create_guild(db, &owner_id.to_raw(), "Test Guild".to_string(), "".to_string())
        .await
        .unwrap();
    (owner_id, guild.id.unwrap().to_raw())
}

#[tokio::test]
async fn create_channel_returns_channel_with_id() {
    let db = common::setup_db().await;
    let (owner_id, guild_id) = setup_guild(&db, "owner1", "owner1@test.com").await;

    let channel = create_channel(&db, &guild_id, &owner_id.to_raw(), "general".to_string(), ChannelType::Text, None)
        .await
        .expect("create_channel failed");

    assert!(channel.id.is_some());
    assert_eq!(channel.name, "general");
}

#[tokio::test]
async fn create_channel_non_member_fails() {
    let db = common::setup_db().await;
    let (_, guild_id) = setup_guild(&db, "owner2", "owner2@test.com").await;
    let outsider = common::create_test_user(&db, "outsider2", "outsider2@test.com").await;

    let result = create_channel(&db, &guild_id, &outsider.to_raw(), "intruder".to_string(), ChannelType::Text, None).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn text_and_voice_channel_types_are_preserved() {
    let db = common::setup_db().await;
    let (owner_id, guild_id) = setup_guild(&db, "owner3", "owner3@test.com").await;

    let text_ch = create_channel(&db, &guild_id, &owner_id.to_raw(), "chat".to_string(), ChannelType::Text, None)
        .await
        .unwrap();
    let voice_ch = create_channel(&db, &guild_id, &owner_id.to_raw(), "voice".to_string(), ChannelType::Voice, None)
        .await
        .unwrap();

    assert!(matches!(text_ch.channel_type, ChannelType::Text));
    assert!(matches!(voice_ch.channel_type, ChannelType::Voice));
}

#[tokio::test]
async fn list_guild_channels_returns_all_channels() {
    let db = common::setup_db().await;
    let (owner_id, guild_id) = setup_guild(&db, "owner4", "owner4@test.com").await;

    create_channel(&db, &guild_id, &owner_id.to_raw(), "general".to_string(), ChannelType::Text, None).await.unwrap();
    create_channel(&db, &guild_id, &owner_id.to_raw(), "lounge".to_string(), ChannelType::Voice, None).await.unwrap();

    let channels = list_guild_channels(&db, &guild_id, &owner_id.to_raw())
        .await
        .expect("list_guild_channels failed");

    assert_eq!(channels.len(), 2);
}

#[tokio::test]
async fn list_guild_channels_non_member_fails() {
    let db = common::setup_db().await;
    let (owner_id, guild_id) = setup_guild(&db, "owner5", "owner5@test.com").await;
    let outsider = common::create_test_user(&db, "outsider5", "outsider5@test.com").await;

    create_channel(&db, &guild_id, &owner_id.to_raw(), "secret".to_string(), ChannelType::Text, None).await.unwrap();

    let result = list_guild_channels(&db, &guild_id, &outsider.to_raw()).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn delete_channel_by_guild_owner_succeeds() {
    let db = common::setup_db().await;
    let (owner_id, guild_id) = setup_guild(&db, "owner6", "owner6@test.com").await;

    let channel = create_channel(&db, &guild_id, &owner_id.to_raw(), "temp".to_string(), ChannelType::Text, None)
        .await
        .unwrap();
    let channel_id = channel.id.unwrap().to_raw();

    delete_channel(&db, &guild_id, &channel_id, &owner_id.to_raw())
        .await
        .expect("delete_channel failed");

    let channels = list_guild_channels(&db, &guild_id, &owner_id.to_raw()).await.unwrap();
    assert!(channels.is_empty());
}

#[tokio::test]
async fn delete_channel_by_non_owner_fails() {
    let db = common::setup_db().await;
    let (owner_id, guild_id) = setup_guild(&db, "owner7", "owner7@test.com").await;
    let member = common::create_test_user(&db, "member7", "member7@test.com").await;

    let channel = create_channel(&db, &guild_id, &owner_id.to_raw(), "general".to_string(), ChannelType::Text, None)
        .await
        .unwrap();
    let channel_id = channel.id.unwrap().to_raw();

    let result = delete_channel(&db, &guild_id, &channel_id, &member.to_raw()).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn channel_category_is_preserved() {
    let db = common::setup_db().await;
    let (owner_id, guild_id) = setup_guild(&db, "owner8", "owner8@test.com").await;

    let channel = create_channel(
        &db,
        &guild_id,
        &owner_id.to_raw(),
        "announcements".to_string(),
        ChannelType::Text,
        Some("INFO".to_string()),
    )
    .await
    .unwrap();

    assert_eq!(channel.category, Some("INFO".to_string()));
}

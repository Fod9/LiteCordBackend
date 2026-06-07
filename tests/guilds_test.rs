mod common;

use litecord_backend::guilds::{
    create_guild, create_invite, delete_guild, join_guild, leave_guild, list_user_guilds,
};

#[tokio::test]
async fn create_guild_returns_guild_with_id() {
    let db = common::setup_db().await;
    let owner_id = common::create_test_user(&db, "owner1", "owner1@test.com").await;

    let guild = create_guild(&db, &owner_id.to_raw(), "Test Guild".to_string(), "".to_string())
        .await
        .expect("create_guild failed");

    assert!(guild.id.is_some());
    assert_eq!(guild.name, "Test Guild");
}

#[tokio::test]
async fn create_guild_owner_is_automatically_member() {
    let db = common::setup_db().await;
    let owner_id = common::create_test_user(&db, "owner2", "owner2@test.com").await;

    let guild = create_guild(&db, &owner_id.to_raw(), "My Guild".to_string(), "".to_string())
        .await
        .unwrap();

    let guilds = list_user_guilds(&db, &owner_id.to_raw()).await.unwrap();
    assert_eq!(guilds.len(), 1);
    assert_eq!(guilds[0].id, guild.id);
}

#[tokio::test]
async fn delete_guild_by_owner_removes_it() {
    let db = common::setup_db().await;
    let owner_id = common::create_test_user(&db, "owner3", "owner3@test.com").await;

    let guild = create_guild(&db, &owner_id.to_raw(), "To Delete".to_string(), "".to_string())
        .await
        .unwrap();

    let guild_id = guild.id.unwrap().to_raw();

    delete_guild(&db, &guild_id, &owner_id.to_raw())
        .await
        .expect("delete_guild failed");

    let guilds = list_user_guilds(&db, &owner_id.to_raw()).await.unwrap();
    assert!(guilds.is_empty());
}

#[tokio::test]
async fn delete_guild_by_non_owner_fails() {
    let db = common::setup_db().await;
    let owner_id = common::create_test_user(&db, "owner4", "owner4@test.com").await;
    let other_id = common::create_test_user(&db, "other4", "other4@test.com").await;

    let guild = create_guild(&db, &owner_id.to_raw(), "Protected".to_string(), "".to_string())
        .await
        .unwrap();

    let guild_id = guild.id.unwrap().to_raw();

    let result = delete_guild(&db, &guild_id, &other_id.to_raw()).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn leave_guild_removes_membership() {
    let db = common::setup_db().await;
    let owner_id = common::create_test_user(&db, "owner5", "owner5@test.com").await;
    let member_id = common::create_test_user(&db, "member5", "member5@test.com").await;

    let guild = create_guild(&db, &owner_id.to_raw(), "Guild5".to_string(), "".to_string())
        .await
        .unwrap();
    let guild_id = guild.id.unwrap().to_raw();

    let invite = create_invite(&db, &guild_id, &owner_id.to_raw()).await.unwrap();
    join_guild(&db, &invite.code, &member_id.to_raw()).await.unwrap();

    let before = list_user_guilds(&db, &member_id.to_raw()).await.unwrap();
    assert_eq!(before.len(), 1);

    leave_guild(&db, &guild_id, &member_id.to_raw())
        .await
        .expect("leave_guild failed");

    let after = list_user_guilds(&db, &member_id.to_raw()).await.unwrap();
    assert!(after.is_empty());
}

#[tokio::test]
async fn owner_cannot_leave_own_guild() {
    let db = common::setup_db().await;
    let owner_id = common::create_test_user(&db, "owner6", "owner6@test.com").await;

    let guild = create_guild(&db, &owner_id.to_raw(), "Guild6".to_string(), "".to_string())
        .await
        .unwrap();
    let guild_id = guild.id.unwrap().to_raw();

    let result = leave_guild(&db, &guild_id, &owner_id.to_raw()).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn create_invite_generates_unique_code() {
    let db = common::setup_db().await;
    let owner_id = common::create_test_user(&db, "owner7", "owner7@test.com").await;

    let guild = create_guild(&db, &owner_id.to_raw(), "Guild7".to_string(), "".to_string())
        .await
        .unwrap();
    let guild_id = guild.id.unwrap().to_raw();

    let invite_a = create_invite(&db, &guild_id, &owner_id.to_raw()).await.unwrap();
    let invite_b = create_invite(&db, &guild_id, &owner_id.to_raw()).await.unwrap();

    assert!(!invite_a.code.is_empty());
    assert_ne!(invite_a.code, invite_b.code);
}

#[tokio::test]
async fn join_guild_via_invite_adds_membership() {
    let db = common::setup_db().await;
    let owner_id = common::create_test_user(&db, "owner8", "owner8@test.com").await;
    let joiner_id = common::create_test_user(&db, "joiner8", "joiner8@test.com").await;

    let guild = create_guild(&db, &owner_id.to_raw(), "Guild8".to_string(), "".to_string())
        .await
        .unwrap();
    let guild_id = guild.id.unwrap().to_raw();

    let invite = create_invite(&db, &guild_id, &owner_id.to_raw()).await.unwrap();
    join_guild(&db, &invite.code, &joiner_id.to_raw())
        .await
        .expect("join_guild failed");

    let guilds = list_user_guilds(&db, &joiner_id.to_raw()).await.unwrap();
    assert_eq!(guilds.len(), 1);
    assert_eq!(guilds[0].name, "Guild8");
}

#[tokio::test]
async fn join_guild_via_invalid_code_fails() {
    let db = common::setup_db().await;
    let user_id = common::create_test_user(&db, "user9", "user9@test.com").await;

    let result = join_guild(&db, "INVALIDCODE", &user_id.to_raw()).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn already_member_cannot_join_again() {
    let db = common::setup_db().await;
    let owner_id = common::create_test_user(&db, "owner10", "owner10@test.com").await;
    let joiner_id = common::create_test_user(&db, "joiner10", "joiner10@test.com").await;

    let guild = create_guild(&db, &owner_id.to_raw(), "Guild10".to_string(), "".to_string())
        .await
        .unwrap();
    let guild_id = guild.id.unwrap().to_raw();

    let invite = create_invite(&db, &guild_id, &owner_id.to_raw()).await.unwrap();
    join_guild(&db, &invite.code, &joiner_id.to_raw()).await.unwrap();

    let invite2 = create_invite(&db, &guild_id, &owner_id.to_raw()).await.unwrap();
    let result = join_guild(&db, &invite2.code, &joiner_id.to_raw()).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn list_guilds_returns_only_user_guilds() {
    let db = common::setup_db().await;
    let owner_a = common::create_test_user(&db, "ownerA", "ownerA@test.com").await;
    let owner_b = common::create_test_user(&db, "ownerB", "ownerB@test.com").await;

    create_guild(&db, &owner_a.to_raw(), "Guild A".to_string(), "".to_string()).await.unwrap();
    create_guild(&db, &owner_b.to_raw(), "Guild B".to_string(), "".to_string()).await.unwrap();

    let guilds_a = list_user_guilds(&db, &owner_a.to_raw()).await.unwrap();
    assert_eq!(guilds_a.len(), 1);
    assert_eq!(guilds_a[0].name, "Guild A");
}

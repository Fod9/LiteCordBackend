mod common;

use litecord_backend::guilds::{
    create_guild, create_invite, delete_guild, join_guild, kick_member, leave_guild,
    list_guild_invites, list_guild_members, list_user_guilds, revoke_invite, update_guild,
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
async fn list_guild_members_returns_owner_and_members() {
    let db = common::setup_db().await;
    let owner_id = common::create_test_user(&db, "ownerM1", "ownerM1@test.com").await;
    let member_id = common::create_test_user(&db, "memberM1", "memberM1@test.com").await;

    let guild = create_guild(&db, &owner_id.to_raw(), "GuildM1".to_string(), "".to_string())
        .await
        .unwrap();
    let guild_id = guild.id.unwrap().to_raw();

    let invite = create_invite(&db, &guild_id, &owner_id.to_raw()).await.unwrap();
    join_guild(&db, &invite.code, &member_id.to_raw()).await.unwrap();

    let members = list_guild_members(&db, &guild_id, &owner_id.to_raw())
        .await
        .expect("list_guild_members failed");

    assert_eq!(members.len(), 2);
    let names: Vec<&str> = members.iter().map(|m| m.user.name.as_str()).collect();
    assert!(names.contains(&"ownerM1"));
    assert!(names.contains(&"memberM1"));
}

#[tokio::test]
async fn list_guild_members_requires_membership() {
    let db = common::setup_db().await;
    let owner_id = common::create_test_user(&db, "ownerM2", "ownerM2@test.com").await;
    let outsider_id = common::create_test_user(&db, "outsiderM2", "outsiderM2@test.com").await;

    let guild = create_guild(&db, &owner_id.to_raw(), "GuildM2".to_string(), "".to_string())
        .await
        .unwrap();
    let guild_id = guild.id.unwrap().to_raw();

    let result = list_guild_members(&db, &guild_id, &outsider_id.to_raw()).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn kick_member_removes_from_guild() {
    let db = common::setup_db().await;
    let owner_id = common::create_test_user(&db, "ownerK1", "ownerK1@test.com").await;
    let member_id = common::create_test_user(&db, "memberK1", "memberK1@test.com").await;

    let guild = create_guild(&db, &owner_id.to_raw(), "GuildK1".to_string(), "".to_string())
        .await
        .unwrap();
    let guild_id = guild.id.unwrap().to_raw();

    let invite = create_invite(&db, &guild_id, &owner_id.to_raw()).await.unwrap();
    join_guild(&db, &invite.code, &member_id.to_raw()).await.unwrap();

    kick_member(&db, &guild_id, &member_id.to_raw(), &owner_id.to_raw())
        .await
        .expect("kick_member failed");

    let guilds = list_user_guilds(&db, &member_id.to_raw()).await.unwrap();
    assert!(guilds.is_empty());
}

#[tokio::test]
async fn kick_member_by_non_owner_fails() {
    let db = common::setup_db().await;
    let owner_id = common::create_test_user(&db, "ownerK2", "ownerK2@test.com").await;
    let member_id = common::create_test_user(&db, "memberK2", "memberK2@test.com").await;
    let other_id = common::create_test_user(&db, "otherK2", "otherK2@test.com").await;

    let guild = create_guild(&db, &owner_id.to_raw(), "GuildK2".to_string(), "".to_string())
        .await
        .unwrap();
    let guild_id = guild.id.unwrap().to_raw();

    let invite = create_invite(&db, &guild_id, &owner_id.to_raw()).await.unwrap();
    join_guild(&db, &invite.code, &member_id.to_raw()).await.unwrap();

    let result = kick_member(&db, &guild_id, &member_id.to_raw(), &other_id.to_raw()).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn update_guild_name_by_owner() {
    let db = common::setup_db().await;
    let owner_id = common::create_test_user(&db, "ownerU1", "ownerU1@test.com").await;

    let guild = create_guild(&db, &owner_id.to_raw(), "OldName".to_string(), "".to_string())
        .await
        .unwrap();
    let guild_id = guild.id.unwrap().to_raw();

    let updated = update_guild(&db, &guild_id, &owner_id.to_raw(), Some("NewName".to_string()), None)
        .await
        .expect("update_guild failed");

    assert_eq!(updated.name, "NewName");
}

#[tokio::test]
async fn update_guild_by_non_owner_fails() {
    let db = common::setup_db().await;
    let owner_id = common::create_test_user(&db, "ownerU2", "ownerU2@test.com").await;
    let other_id = common::create_test_user(&db, "otherU2", "otherU2@test.com").await;

    let guild = create_guild(&db, &owner_id.to_raw(), "Guild".to_string(), "".to_string())
        .await
        .unwrap();
    let guild_id = guild.id.unwrap().to_raw();

    let result = update_guild(&db, &guild_id, &other_id.to_raw(), Some("Hack".to_string()), None).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn list_guild_invites_returns_all_invites() {
    let db = common::setup_db().await;
    let owner_id = common::create_test_user(&db, "ownerI1", "ownerI1@test.com").await;

    let guild = create_guild(&db, &owner_id.to_raw(), "GuildI1".to_string(), "".to_string())
        .await
        .unwrap();
    let guild_id = guild.id.unwrap().to_raw();

    create_invite(&db, &guild_id, &owner_id.to_raw()).await.unwrap();
    create_invite(&db, &guild_id, &owner_id.to_raw()).await.unwrap();

    let invites = list_guild_invites(&db, &guild_id, &owner_id.to_raw())
        .await
        .expect("list_guild_invites failed");

    assert_eq!(invites.len(), 2);
}

#[tokio::test]
async fn revoke_invite_removes_it() {
    let db = common::setup_db().await;
    let owner_id = common::create_test_user(&db, "ownerI2", "ownerI2@test.com").await;

    let guild = create_guild(&db, &owner_id.to_raw(), "GuildI2".to_string(), "".to_string())
        .await
        .unwrap();
    let guild_id = guild.id.unwrap().to_raw();

    let invite = create_invite(&db, &guild_id, &owner_id.to_raw()).await.unwrap();
    let invite_id = invite.id.unwrap().to_raw();

    revoke_invite(&db, &guild_id, &invite_id, &owner_id.to_raw())
        .await
        .expect("revoke_invite failed");

    let invites = list_guild_invites(&db, &guild_id, &owner_id.to_raw()).await.unwrap();
    assert!(invites.is_empty());
}

#[tokio::test]
async fn delete_guild_returns_member_ids() {
    let db = common::setup_db().await;
    let owner_id = common::create_test_user(&db, "ownerDR1", "ownerDR1@test.com").await;
    let member_id = common::create_test_user(&db, "memberDR1", "memberDR1@test.com").await;

    let guild = create_guild(&db, &owner_id.to_raw(), "ToDeleteR".to_string(), "".to_string())
        .await
        .unwrap();
    let guild_id = guild.id.unwrap().to_raw();

    let invite = create_invite(&db, &guild_id, &owner_id.to_raw()).await.unwrap();
    join_guild(&db, &invite.code, &member_id.to_raw()).await.unwrap();

    let member_ids = delete_guild(&db, &guild_id, &owner_id.to_raw())
        .await
        .expect("delete_guild failed");

    assert_eq!(member_ids.len(), 2);
    assert!(member_ids.contains(&owner_id.to_raw()));
    assert!(member_ids.contains(&member_id.to_raw()));
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

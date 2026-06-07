mod common;

use litecord_backend::guilds::create_guild;
use litecord_backend::roles::{assign_role, check_permission, create_role, delete_role, list_roles, remove_role};

async fn setup_guild(
    db: &surrealdb::Surreal<surrealdb::engine::any::Any>,
) -> (surrealdb::sql::Thing, surrealdb::sql::Thing) {
    let owner_id = common::create_test_user(db, "owner", "owner@test.com").await;
    let guild = create_guild(db, &owner_id.to_raw(), "Test Guild".to_string(), "icon".to_string())
        .await
        .unwrap();
    let guild_id = guild.id.unwrap();
    (owner_id, guild_id)
}

#[tokio::test]
async fn create_role_by_owner_succeeds() {
    let db = common::setup_db().await;
    let (owner_id, guild_id) = setup_guild(&db).await;

    let result = create_role(
        &db,
        &guild_id.to_raw(),
        &owner_id.to_raw(),
        "Moderator".to_string(),
        "#ff0000".to_string(),
        1,
        vec!["kick_members".to_string()],
    )
    .await;

    assert!(result.is_ok(), "owner should be able to create a role: {:?}", result);
    let role = result.unwrap();
    assert!(role.id.is_some());
    assert_eq!(role.name, "Moderator");
    assert_eq!(role.permissions, vec!["kick_members"]);
}

#[tokio::test]
async fn create_role_by_non_owner_fails() {
    let db = common::setup_db().await;
    let (_, guild_id) = setup_guild(&db).await;
    let other_id = common::create_test_user(&db, "other", "other@test.com").await;

    let result = create_role(
        &db,
        &guild_id.to_raw(),
        &other_id.to_raw(),
        "Role".to_string(),
        "#000000".to_string(),
        1,
        vec![],
    )
    .await;

    assert!(result.is_err());
    let (status, _) = result.unwrap_err();
    assert_eq!(status, rocket::http::Status::Forbidden);
}

#[tokio::test]
async fn list_roles_returns_guild_roles() {
    let db = common::setup_db().await;
    let (owner_id, guild_id) = setup_guild(&db).await;

    create_role(&db, &guild_id.to_raw(), &owner_id.to_raw(), "Mod".to_string(), "#ff0000".to_string(), 1, vec![]).await.unwrap();
    create_role(&db, &guild_id.to_raw(), &owner_id.to_raw(), "Helper".to_string(), "#00ff00".to_string(), 2, vec![]).await.unwrap();

    let roles = list_roles(&db, &guild_id.to_raw()).await.unwrap();
    assert_eq!(roles.len(), 2);
}

#[tokio::test]
async fn assign_role_to_member_succeeds() {
    let db = common::setup_db().await;
    let (owner_id, guild_id) = setup_guild(&db).await;

    let role = create_role(&db, &guild_id.to_raw(), &owner_id.to_raw(), "Mod".to_string(), "#ff0000".to_string(), 1, vec![]).await.unwrap();
    let role_id = role.id.unwrap();

    let member_id = common::create_test_user(&db, "member", "member@test.com").await;
    litecord_backend::guilds::join_guild_directly(&db, &guild_id, &member_id).await.unwrap();

    let result = assign_role(&db, &guild_id.to_raw(), &role_id.to_raw(), &member_id.to_raw(), &owner_id.to_raw()).await;
    assert!(result.is_ok(), "owner should be able to assign a role: {:?}", result);
}

#[tokio::test]
async fn assign_role_to_non_member_fails() {
    let db = common::setup_db().await;
    let (owner_id, guild_id) = setup_guild(&db).await;

    let role = create_role(&db, &guild_id.to_raw(), &owner_id.to_raw(), "Mod".to_string(), "#ff0000".to_string(), 1, vec![]).await.unwrap();
    let role_id = role.id.unwrap();

    let outsider_id = common::create_test_user(&db, "outsider", "outsider@test.com").await;

    let result = assign_role(&db, &guild_id.to_raw(), &role_id.to_raw(), &outsider_id.to_raw(), &owner_id.to_raw()).await;
    assert!(result.is_err());
    let (status, _) = result.unwrap_err();
    assert_eq!(status, rocket::http::Status::NotFound);
}

#[tokio::test]
async fn check_permission_with_assigned_role_returns_true() {
    let db = common::setup_db().await;
    let (owner_id, guild_id) = setup_guild(&db).await;

    let role = create_role(
        &db,
        &guild_id.to_raw(),
        &owner_id.to_raw(),
        "Mod".to_string(),
        "#ff0000".to_string(),
        1,
        vec!["kick_members".to_string()],
    )
    .await
    .unwrap();
    let role_id = role.id.unwrap();

    let member_id = common::create_test_user(&db, "member2", "member2@test.com").await;
    litecord_backend::guilds::join_guild_directly(&db, &guild_id, &member_id).await.unwrap();
    assign_role(&db, &guild_id.to_raw(), &role_id.to_raw(), &member_id.to_raw(), &owner_id.to_raw()).await.unwrap();

    let has_perm = check_permission(&db, &guild_id.to_raw(), &member_id.to_raw(), "kick_members").await.unwrap();
    assert!(has_perm);
}

#[tokio::test]
async fn check_permission_without_role_returns_false() {
    let db = common::setup_db().await;
    let (owner_id, guild_id) = setup_guild(&db).await;

    let member_id = common::create_test_user(&db, "member3", "member3@test.com").await;
    litecord_backend::guilds::join_guild_directly(&db, &guild_id, &member_id).await.unwrap();

    let has_perm = check_permission(&db, &guild_id.to_raw(), &member_id.to_raw(), "kick_members").await.unwrap();
    assert!(!has_perm);
}

#[tokio::test]
async fn remove_role_from_member_succeeds() {
    let db = common::setup_db().await;
    let (owner_id, guild_id) = setup_guild(&db).await;

    let role = create_role(&db, &guild_id.to_raw(), &owner_id.to_raw(), "Mod".to_string(), "#ff0000".to_string(), 1, vec!["kick_members".to_string()]).await.unwrap();
    let role_id = role.id.unwrap();

    let member_id = common::create_test_user(&db, "member4", "member4@test.com").await;
    litecord_backend::guilds::join_guild_directly(&db, &guild_id, &member_id).await.unwrap();
    assign_role(&db, &guild_id.to_raw(), &role_id.to_raw(), &member_id.to_raw(), &owner_id.to_raw()).await.unwrap();

    let result = remove_role(&db, &guild_id.to_raw(), &role_id.to_raw(), &member_id.to_raw(), &owner_id.to_raw()).await;
    assert!(result.is_ok(), "remove_role should succeed: {:?}", result);

    let has_perm = check_permission(&db, &guild_id.to_raw(), &member_id.to_raw(), "kick_members").await.unwrap();
    assert!(!has_perm, "permission should be gone after role removal");
}

#[tokio::test]
async fn delete_role_by_owner_succeeds() {
    let db = common::setup_db().await;
    let (owner_id, guild_id) = setup_guild(&db).await;

    let role = create_role(&db, &guild_id.to_raw(), &owner_id.to_raw(), "Mod".to_string(), "#ff0000".to_string(), 1, vec![]).await.unwrap();
    let role_id = role.id.unwrap();

    let result = delete_role(&db, &guild_id.to_raw(), &role_id.to_raw(), &owner_id.to_raw()).await;
    assert!(result.is_ok(), "owner should be able to delete a role: {:?}", result);

    let roles = list_roles(&db, &guild_id.to_raw()).await.unwrap();
    assert_eq!(roles.len(), 0);
}

#[tokio::test]
async fn delete_role_by_non_owner_fails() {
    let db = common::setup_db().await;
    let (owner_id, guild_id) = setup_guild(&db).await;

    let role = create_role(&db, &guild_id.to_raw(), &owner_id.to_raw(), "Mod".to_string(), "#ff0000".to_string(), 1, vec![]).await.unwrap();
    let role_id = role.id.unwrap();

    let other_id = common::create_test_user(&db, "other2", "other2@test.com").await;

    let result = delete_role(&db, &guild_id.to_raw(), &role_id.to_raw(), &other_id.to_raw()).await;
    assert!(result.is_err());
    let (status, _) = result.unwrap_err();
    assert_eq!(status, rocket::http::Status::Forbidden);
}

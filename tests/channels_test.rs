mod common;

use litecord_backend::channels::{create_dm_channel, list_channels_for_user};

#[tokio::test]
async fn list_dm_channels_embeds_participant_objects() {
    let db = common::setup_db().await;
    let user_a_id = common::create_test_user(&db, "ch_user_a", "ch_user_a@test.com").await;
    let user_b_id = common::create_test_user(&db, "ch_user_b", "ch_user_b@test.com").await;

    let mut sorted = vec![user_a_id.to_raw(), user_b_id.to_raw()];
    sorted.sort();
    let recipients_key = sorted.join("_");

    db.query("CREATE DMChannel SET recipients = $recipients, owner = $owner, recipients_key = $rk")
        .bind(("recipients", vec![user_a_id.clone(), user_b_id.clone()]))
        .bind(("owner", user_a_id.clone()))
        .bind(("rk", recipients_key))
        .await
        .unwrap();

    let (channels, _) = list_channels_for_user(&user_a_id.to_raw(), &db).await.unwrap();

    assert_eq!(channels.len(), 1);
    let channel = &channels[0];
    assert_eq!(channel.participants.len(), 2);

    let names: Vec<&str> = channel.participants.iter().map(|p| p.name.as_str()).collect();
    assert!(names.contains(&"ch_user_a"), "participants must include ch_user_a");
    assert!(names.contains(&"ch_user_b"), "participants must include ch_user_b");
    assert!(
        channel.participants.iter().all(|p| p.id.to_raw().starts_with("user:")),
        "each participant must have a full user id"
    );
}

#[tokio::test]
async fn create_dm_channel_returns_channel_with_participants() {
    let db = common::setup_db().await;
    let owner_id = common::create_test_user(&db, "dm_owner", "dm_owner@test.com").await;
    let recipient_id = common::create_test_user(&db, "dm_recipient", "dm_recipient@test.com").await;
    common::create_accepted_friendship(&db, &owner_id, &recipient_id).await;

    let channel = create_dm_channel(
        &owner_id.to_raw(),
        vec![recipient_id.to_raw()],
        &db,
    )
    .await
    .expect("create_dm_channel should succeed");

    assert!(channel.id.is_some());
    assert_eq!(channel.participants.len(), 2);

    let names: Vec<&str> = channel.participants.iter().map(|p| p.name.as_str()).collect();
    assert!(names.contains(&"dm_owner"));
    assert!(names.contains(&"dm_recipient"));
}

#[tokio::test]
async fn create_dm_channel_is_idempotent() {
    let db = common::setup_db().await;
    let owner_id = common::create_test_user(&db, "idem_owner", "idem_owner@test.com").await;
    let recipient_id = common::create_test_user(&db, "idem_recip", "idem_recip@test.com").await;
    common::create_accepted_friendship(&db, &owner_id, &recipient_id).await;

    let first = create_dm_channel(&owner_id.to_raw(), vec![recipient_id.to_raw()], &db)
        .await
        .unwrap();
    let second = create_dm_channel(&owner_id.to_raw(), vec![recipient_id.to_raw()], &db)
        .await
        .unwrap();

    assert_eq!(
        first.id.unwrap().to_raw(),
        second.id.unwrap().to_raw(),
        "same recipients must return the same channel"
    );
}

#[tokio::test]
async fn create_group_dm_channel_includes_all_participants() {
    let db = common::setup_db().await;
    let owner_id = common::create_test_user(&db, "grp_owner", "grp_owner@test.com").await;
    let user_b_id = common::create_test_user(&db, "grp_b", "grp_b@test.com").await;
    let user_c_id = common::create_test_user(&db, "grp_c", "grp_c@test.com").await;
    common::create_accepted_friendship(&db, &owner_id, &user_b_id).await;
    common::create_accepted_friendship(&db, &owner_id, &user_c_id).await;

    let channel = create_dm_channel(
        &owner_id.to_raw(),
        vec![user_b_id.to_raw(), user_c_id.to_raw()],
        &db,
    )
    .await
    .expect("group dm creation should succeed");

    assert_eq!(channel.participants.len(), 3);
}

#[tokio::test]
async fn create_dm_channel_fails_without_friendship() {
    let db = common::setup_db().await;
    let owner_id = common::create_test_user(&db, "nofriend_owner", "nofriend_owner@test.com").await;
    let recipient_id = common::create_test_user(&db, "nofriend_recip", "nofriend_recip@test.com").await;

    let result = create_dm_channel(&owner_id.to_raw(), vec![recipient_id.to_raw()], &db).await;

    assert!(result.is_err());
    let (status, _) = result.unwrap_err();
    assert_eq!(status, rocket::http::Status::Forbidden);
}

#[tokio::test]
async fn create_dm_channel_fails_with_pending_friendship() {
    let db = common::setup_db().await;
    let owner_id = common::create_test_user(&db, "pending_owner", "pending_owner@test.com").await;
    common::create_test_user(&db, "pending_recip", "pending_recip@test.com").await;

    litecord_backend::friends::add_friend(owner_id.to_raw(), "pending_recip".to_string(), &db)
        .await
        .unwrap();

    let recipient_id = db
        .query("SELECT * FROM user WHERE name = 'pending_recip'")
        .await
        .unwrap()
        .take::<Vec<litecord_backend::models::db::User>>(0)
        .unwrap()
        .into_iter()
        .next()
        .unwrap()
        .id
        .unwrap();

    let result = create_dm_channel(&owner_id.to_raw(), vec![recipient_id.to_raw()], &db).await;

    assert!(result.is_err());
    let (status, _) = result.unwrap_err();
    assert_eq!(status, rocket::http::Status::Forbidden);
}

#[tokio::test]
async fn create_dm_channel_with_invalid_recipient_fails() {
    let db = common::setup_db().await;
    let owner_id = common::create_test_user(&db, "err_owner", "err_owner@test.com").await;

    let result = create_dm_channel(
        &owner_id.to_raw(),
        vec!["user:doesnotexist".to_string()],
        &db,
    )
    .await;

    assert!(result.is_err());
}

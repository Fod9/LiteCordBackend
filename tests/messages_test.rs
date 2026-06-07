mod common;

use litecord_backend::messages::{get_channel_messages, save_message};

#[tokio::test]
async fn save_message_persists_and_returns_id() {
    let db = common::setup_db().await;
    let user_id = common::create_test_user(&db, "alice", "alice@test.com").await;
    let channel_id = common::create_test_dm_channel(&db, &user_id).await;

    let msg = save_message(&db, &channel_id, &user_id.to_raw(), "hello world")
        .await
        .expect("save_message failed");

    assert!(msg.id.is_some());
    assert_eq!(msg.content, "hello world");
    assert!(msg.attachments.is_empty());
}

#[tokio::test]
async fn get_channel_messages_returns_chronological_order() {
    let db = common::setup_db().await;
    let user_id = common::create_test_user(&db, "bob", "bob@test.com").await;
    let channel_id = common::create_test_dm_channel(&db, &user_id).await;

    save_message(&db, &channel_id, &user_id.to_raw(), "first").await.unwrap();
    save_message(&db, &channel_id, &user_id.to_raw(), "second").await.unwrap();
    save_message(&db, &channel_id, &user_id.to_raw(), "third").await.unwrap();

    let messages = get_channel_messages(&db, &channel_id, 50, None)
        .await
        .expect("get_channel_messages failed");

    assert_eq!(messages.len(), 3);
    assert_eq!(messages[0].content, "first");
    assert_eq!(messages[1].content, "second");
    assert_eq!(messages[2].content, "third");
}

#[tokio::test]
async fn get_channel_messages_respects_limit() {
    let db = common::setup_db().await;
    let user_id = common::create_test_user(&db, "carol", "carol@test.com").await;
    let channel_id = common::create_test_dm_channel(&db, &user_id).await;

    for i in 0..10 {
        save_message(&db, &channel_id, &user_id.to_raw(), &format!("msg {i}"))
            .await
            .unwrap();
    }

    let messages = get_channel_messages(&db, &channel_id, 3, None)
        .await
        .expect("get_channel_messages failed");

    assert_eq!(messages.len(), 3);
}

#[tokio::test]
async fn get_channel_messages_cursor_returns_only_older_messages() {
    let db = common::setup_db().await;
    let user_id = common::create_test_user(&db, "dave", "dave@test.com").await;
    let channel_id = common::create_test_dm_channel(&db, &user_id).await;

    save_message(&db, &channel_id, &user_id.to_raw(), "old").await.unwrap();
    let pivot = save_message(&db, &channel_id, &user_id.to_raw(), "pivot").await.unwrap();
    save_message(&db, &channel_id, &user_id.to_raw(), "new").await.unwrap();

    let pivot_id = pivot.id.unwrap().to_raw();

    let older = get_channel_messages(&db, &channel_id, 50, Some(pivot_id))
        .await
        .expect("cursor query failed");

    assert_eq!(older.len(), 1);
    assert_eq!(older[0].content, "old");
}

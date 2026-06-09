mod common;

use litecord_backend::messages::{get_channel_messages, save_message, save_message_with_author};
use litecord_backend::models::db::Attachment;

#[tokio::test]
async fn save_message_persists_and_returns_id() {
    let db = common::setup_db().await;
    let user_id = common::create_test_user(&db, "alice", "alice@test.com").await;
    let channel_id = common::create_test_dm_channel(&db, &user_id).await;

    let msg = save_message(&db, &channel_id, &user_id.to_raw(), "hello world", vec![])
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

    save_message(&db, &channel_id, &user_id.to_raw(), "first", vec![]).await.unwrap();
    save_message(&db, &channel_id, &user_id.to_raw(), "second", vec![]).await.unwrap();
    save_message(&db, &channel_id, &user_id.to_raw(), "third", vec![]).await.unwrap();

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
        save_message(&db, &channel_id, &user_id.to_raw(), &format!("msg {i}"), vec![])
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

    save_message(&db, &channel_id, &user_id.to_raw(), "old", vec![]).await.unwrap();
    let pivot = save_message(&db, &channel_id, &user_id.to_raw(), "pivot", vec![]).await.unwrap();
    save_message(&db, &channel_id, &user_id.to_raw(), "new", vec![]).await.unwrap();

    let pivot_id = pivot.id.unwrap().to_raw();

    let older = get_channel_messages(&db, &channel_id, 50, Some(pivot_id))
        .await
        .expect("cursor query failed");

    assert_eq!(older.len(), 1);
    assert_eq!(older[0].content, "old");
}

#[tokio::test]
async fn get_channel_messages_includes_author_profile() {
    let db = common::setup_db().await;
    let user_id = common::create_test_user(&db, "eve", "eve@test.com").await;
    let channel_id = common::create_test_dm_channel(&db, &user_id).await;

    save_message(&db, &channel_id, &user_id.to_raw(), "hello", vec![])
        .await
        .unwrap();

    let messages = get_channel_messages(&db, &channel_id, 50, None)
        .await
        .expect("get_channel_messages failed");

    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].author.name, "eve");
}

#[tokio::test]
async fn save_message_with_author_returns_enriched_message() {
    let db = common::setup_db().await;
    let user_id = common::create_test_user(&db, "frank", "frank@test.com").await;
    let channel_id = common::create_test_dm_channel(&db, &user_id).await;

    let msg = save_message_with_author(&db, &channel_id, &user_id.to_raw(), "hi", vec![])
        .await
        .expect("save_message_with_author failed");

    assert!(msg.id.is_some());
    assert_eq!(msg.content, "hi");
    assert_eq!(msg.author.name, "frank");
}

#[tokio::test]
async fn save_message_with_attachments_persists_them() {
    let db = common::setup_db().await;
    let user_id = common::create_test_user(&db, "attach_user", "attach_user@test.com").await;
    let channel_id = common::create_test_dm_channel(&db, &user_id).await;

    let attachments = vec![Attachment {
        url: "https://cdn.example.com/uuid/photo.jpg".to_string(),
        filename: "photo.jpg".to_string(),
        size: 98765,
    }];

    let msg = save_message(&db, &channel_id, &user_id.to_raw(), "see attachment", attachments)
        .await
        .expect("save_message with attachments failed");

    assert_eq!(msg.attachments.len(), 1);
    assert_eq!(msg.attachments[0].filename, "photo.jpg");
    assert_eq!(msg.attachments[0].size, 98765);
    assert_eq!(msg.attachments[0].url, "https://cdn.example.com/uuid/photo.jpg");
}

#[tokio::test]
async fn save_message_with_multiple_attachments() {
    let db = common::setup_db().await;
    let user_id = common::create_test_user(&db, "attach_multi", "attach_multi@test.com").await;
    let channel_id = common::create_test_dm_channel(&db, &user_id).await;

    let attachments = vec![
        Attachment { url: "https://cdn.example.com/a/file1.png".to_string(), filename: "file1.png".to_string(), size: 1000 },
        Attachment { url: "https://cdn.example.com/b/file2.mp3".to_string(), filename: "file2.mp3".to_string(), size: 500000 },
    ];

    let msg = save_message(&db, &channel_id, &user_id.to_raw(), "", attachments)
        .await
        .expect("save_message with multiple attachments failed");

    assert_eq!(msg.attachments.len(), 2);
    assert_eq!(msg.attachments[1].filename, "file2.mp3");
}

#[tokio::test]
async fn get_channel_messages_returns_attachments() {
    let db = common::setup_db().await;
    let user_id = common::create_test_user(&db, "attach_fetch", "attach_fetch@test.com").await;
    let channel_id = common::create_test_dm_channel(&db, &user_id).await;

    let attachments = vec![Attachment {
        url: "https://cdn.example.com/x/doc.pdf".to_string(),
        filename: "doc.pdf".to_string(),
        size: 204800,
    }];
    save_message(&db, &channel_id, &user_id.to_raw(), "here", attachments).await.unwrap();

    let messages = get_channel_messages(&db, &channel_id, 10, None).await.unwrap();
    assert_eq!(messages[0].attachments.len(), 1);
    assert_eq!(messages[0].attachments[0].filename, "doc.pdf");
}

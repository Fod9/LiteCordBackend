mod common;

use std::sync::Arc;
use litecord_backend::chat::hub::ChatHub;
use litecord_backend::friends::{add_friend, update_friend_request};
use litecord_backend::models::db::Friendship;
use rocket::tokio::sync::broadcast;

#[tokio::test]
async fn broadcast_presence_online_notifies_connected_friend() {
    let db = common::setup_db().await;
    let user_a = common::create_test_user(&db, "pres_a", "pres_a@test.com").await;
    let user_b = common::create_test_user(&db, "pres_b", "pres_b@test.com").await;

    let friendship = add_friend(user_a.to_raw(), "pres_b".to_string(), &db).await.unwrap();
    let fid = friendship.id.unwrap().to_raw();
    update_friend_request(user_b.to_raw(), fid, &db, "accept").await.unwrap();

    let hub = Arc::new(ChatHub::new());
    let (tx_a, mut rx_a) = broadcast::channel(10);
    hub.connections.write().await.insert(user_a.to_raw(), tx_a);

    let online = hub.broadcast_presence(&db, &user_b.to_raw(), true).await;

    let msg = rx_a.try_recv().expect("user_a should receive online notification");
    let v: serde_json::Value = serde_json::from_str(&msg).unwrap();
    assert_eq!(v["message_type"], "user_online");
    assert_eq!(v["user_id"], user_b.to_raw());

    assert!(online.contains(&user_a.to_raw()), "online list must include connected friend user_a");
}

#[tokio::test]
async fn broadcast_presence_offline_notifies_connected_friend() {
    let db = common::setup_db().await;
    let user_c = common::create_test_user(&db, "pres_c", "pres_c@test.com").await;
    let user_d = common::create_test_user(&db, "pres_d", "pres_d@test.com").await;

    let friendship = add_friend(user_c.to_raw(), "pres_d".to_string(), &db).await.unwrap();
    let fid = friendship.id.unwrap().to_raw();
    update_friend_request(user_d.to_raw(), fid, &db, "accept").await.unwrap();

    let hub = Arc::new(ChatHub::new());
    let (tx_c, mut rx_c) = broadcast::channel(10);
    hub.connections.write().await.insert(user_c.to_raw(), tx_c);

    hub.broadcast_presence(&db, &user_d.to_raw(), false).await;

    let msg = rx_c.try_recv().expect("user_c should receive offline notification");
    let v: serde_json::Value = serde_json::from_str(&msg).unwrap();
    assert_eq!(v["message_type"], "user_offline");
    assert_eq!(v["user_id"], user_d.to_raw());
}

#[tokio::test]
async fn broadcast_presence_does_not_notify_non_friends() {
    let db = common::setup_db().await;
    let user_e = common::create_test_user(&db, "pres_e", "pres_e@test.com").await;
    let user_f = common::create_test_user(&db, "pres_f", "pres_f@test.com").await;

    // No friendship between e and f
    let hub = Arc::new(ChatHub::new());
    let (tx_e, mut rx_e) = broadcast::channel(10);
    hub.connections.write().await.insert(user_e.to_raw(), tx_e);

    hub.broadcast_presence(&db, &user_f.to_raw(), true).await;

    assert!(rx_e.try_recv().is_err(), "non-friend must not receive presence event");
}

#[tokio::test]
async fn broadcast_presence_returns_only_connected_friends() {
    let db = common::setup_db().await;
    let user_g = common::create_test_user(&db, "pres_g", "pres_g@test.com").await;
    let user_h = common::create_test_user(&db, "pres_h", "pres_h@test.com").await;
    let user_i = common::create_test_user(&db, "pres_i", "pres_i@test.com").await;

    // user_h and user_i are both friends of user_g
    let f1 = add_friend(user_g.to_raw(), "pres_h".to_string(), &db).await.unwrap();
    update_friend_request(user_h.to_raw(), f1.id.unwrap().to_raw(), &db, "accept").await.unwrap();
    let f2 = add_friend(user_g.to_raw(), "pres_i".to_string(), &db).await.unwrap();
    update_friend_request(user_i.to_raw(), f2.id.unwrap().to_raw(), &db, "accept").await.unwrap();

    let hub = Arc::new(ChatHub::new());
    // Only user_h is connected, user_i is not
    let (tx_h, _rx_h) = broadcast::channel(10);
    hub.connections.write().await.insert(user_h.to_raw(), tx_h);

    let online = hub.broadcast_presence(&db, &user_g.to_raw(), true).await;

    assert!(online.contains(&user_h.to_raw()), "connected friend user_h must appear in online list");
    assert!(!online.contains(&user_i.to_raw()), "disconnected friend user_i must not appear");
}

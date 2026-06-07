mod common;

use litecord_backend::friends::{add_friend, list_friends, list_pending_requests, remove_friend, update_friend_request};
use litecord_backend::models::db::{Friendship, FriendshipWithUsers};

#[tokio::test]
async fn add_friend_creates_pending_friendship() {
    let db = common::setup_db().await;
    let user_a_id = common::create_test_user(&db, "user_a", "user_a@test.com").await;
    common::create_test_user(&db, "user_b", "user_b@test.com").await;

    let result = add_friend(user_a_id.to_raw(), "user_b".to_string(), &db).await;
    assert!(result.is_ok(), "add_friend should succeed: {:?}", result);

    let accepted = list_friends(user_a_id.to_raw(), &db).await.unwrap();
    assert_eq!(accepted.len(), 0, "pending friendship must not appear in accepted list");
}

#[tokio::test]
async fn add_friend_to_unknown_user_returns_not_found() {
    let db = common::setup_db().await;
    let user_id = common::create_test_user(&db, "user_c", "user_c@test.com").await;

    let result = add_friend(user_id.to_raw(), "nonexistent_user".to_string(), &db).await;
    assert!(result.is_err());

    let (status, _) = result.unwrap_err();
    assert_eq!(status, rocket::http::Status::NotFound);
}

#[tokio::test]
async fn add_friend_duplicate_returns_error() {
    let db = common::setup_db().await;
    let user_d_id = common::create_test_user(&db, "user_d", "user_d@test.com").await;
    common::create_test_user(&db, "user_e", "user_e@test.com").await;

    add_friend(user_d_id.to_raw(), "user_e".to_string(), &db).await.unwrap();

    let duplicate = add_friend(user_d_id.to_raw(), "user_e".to_string(), &db).await;
    assert!(duplicate.is_err(), "duplicate friend request should fail");
}

#[tokio::test]
async fn accept_friend_request_appears_in_list() {
    let db = common::setup_db().await;
    let user_f_id = common::create_test_user(&db, "user_f", "user_f@test.com").await;
    let user_g_id = common::create_test_user(&db, "user_g", "user_g@test.com").await;

    add_friend(user_f_id.to_raw(), "user_g".to_string(), &db).await.unwrap();

    let friendships: Vec<Friendship> = db
        .query("SELECT * FROM friendship WHERE status = 'pending'")
        .await
        .unwrap()
        .take(0)
        .unwrap();
    assert_eq!(friendships.len(), 1);

    let friendship_id = friendships[0].id.as_ref().unwrap().to_raw();

    update_friend_request(user_g_id.to_raw(), friendship_id, &db, "accept")
        .await
        .expect("accept should succeed");

    let accepted = list_friends(user_f_id.to_raw(), &db).await.unwrap();
    assert_eq!(accepted.len(), 1);
}

#[tokio::test]
async fn add_friend_returns_friendship_with_correct_users() {
    let db = common::setup_db().await;
    let user_p_id = common::create_test_user(&db, "user_p", "user_p@test.com").await;
    common::create_test_user(&db, "user_q", "user_q@test.com").await;

    let result = add_friend(user_p_id.to_raw(), "user_q".to_string(), &db).await;
    assert!(result.is_ok(), "add_friend should succeed: {:?}", result);

    let friendship = result.unwrap();
    assert!(friendship.id.is_some(), "returned friendship must have an id");
    assert_eq!(friendship.status, "pending");
    assert_eq!(friendship.user_a.to_raw(), user_p_id.to_raw(), "sender must be user_a");
}

#[tokio::test]
async fn update_friend_request_returns_updated_friendship() {
    let db = common::setup_db().await;
    let user_r_id = common::create_test_user(&db, "user_r", "user_r@test.com").await;
    let user_s_id = common::create_test_user(&db, "user_s", "user_s@test.com").await;

    add_friend(user_r_id.to_raw(), "user_s".to_string(), &db).await.unwrap();

    let friendships: Vec<Friendship> = db
        .query("SELECT * FROM friendship WHERE status = 'pending'")
        .await.unwrap().take(0).unwrap();
    let friendship_id = friendships[0].id.as_ref().unwrap().to_raw();

    let result = update_friend_request(user_s_id.to_raw(), friendship_id, &db, "accept").await;
    assert!(result.is_ok(), "update should succeed: {:?}", result);

    let updated = result.unwrap();
    assert_eq!(updated.status, "accepted");
}

#[tokio::test]
async fn list_pending_requests_shows_received_requests() {
    let db = common::setup_db().await;
    let user_j_id = common::create_test_user(&db, "user_j", "user_j@test.com").await;
    let user_k_id = common::create_test_user(&db, "user_k", "user_k@test.com").await;

    add_friend(user_j_id.to_raw(), "user_k".to_string(), &db).await.unwrap();

    let pending = list_pending_requests(user_k_id.to_raw(), &db).await.unwrap();
    assert_eq!(pending.len(), 1, "user_k should see the pending request sent by user_j");
}

#[tokio::test]
async fn list_pending_requests_does_not_show_sent_requests() {
    let db = common::setup_db().await;
    let user_l_id = common::create_test_user(&db, "user_l", "user_l@test.com").await;
    common::create_test_user(&db, "user_m", "user_m@test.com").await;

    add_friend(user_l_id.to_raw(), "user_m".to_string(), &db).await.unwrap();

    let pending = list_pending_requests(user_l_id.to_raw(), &db).await.unwrap();
    assert_eq!(pending.len(), 0, "sender should not see their own sent request as pending");
}

#[tokio::test]
async fn list_pending_requests_empty_after_accept() {
    let db = common::setup_db().await;
    let user_n_id = common::create_test_user(&db, "user_n", "user_n@test.com").await;
    let user_o_id = common::create_test_user(&db, "user_o", "user_o@test.com").await;

    add_friend(user_n_id.to_raw(), "user_o".to_string(), &db).await.unwrap();

    let friendships: Vec<Friendship> = db
        .query("SELECT * FROM friendship WHERE status = 'pending'")
        .await.unwrap().take(0).unwrap();
    let friendship_id = friendships[0].id.as_ref().unwrap().to_raw();

    update_friend_request(user_o_id.to_raw(), friendship_id, &db, "accept").await.unwrap();

    let pending = list_pending_requests(user_o_id.to_raw(), &db).await.unwrap();
    assert_eq!(pending.len(), 0, "no pending requests after acceptance");
}

#[tokio::test]
async fn list_friends_embeds_user_objects() {
    let db = common::setup_db().await;
    let user_y_id = common::create_test_user(&db, "user_y", "user_y@test.com").await;
    let user_z_id = common::create_test_user(&db, "user_z", "user_z@test.com").await;

    let friendship = add_friend(user_y_id.to_raw(), "user_z".to_string(), &db).await.unwrap();
    let friendship_id = friendship.id.unwrap().to_raw();
    update_friend_request(user_z_id.to_raw(), friendship_id, &db, "accept").await.unwrap();

    let friends: Vec<FriendshipWithUsers> = list_friends(user_y_id.to_raw(), &db).await.unwrap();
    assert_eq!(friends.len(), 1);

    let f = &friends[0];
    assert_eq!(f.in_user.name, "user_y");
    assert_eq!(f.out_user.name, "user_z");
    assert!(f.in_user.id.to_raw().starts_with("user:"));
}

#[tokio::test]
async fn list_pending_requests_embeds_user_objects() {
    let db = common::setup_db().await;
    let user_aa_id = common::create_test_user(&db, "user_aa", "user_aa@test.com").await;
    let user_bb_id = common::create_test_user(&db, "user_bb", "user_bb@test.com").await;

    add_friend(user_aa_id.to_raw(), "user_bb".to_string(), &db).await.unwrap();

    let pending: Vec<FriendshipWithUsers> = list_pending_requests(user_bb_id.to_raw(), &db).await.unwrap();
    assert_eq!(pending.len(), 1);

    let f = &pending[0];
    assert_eq!(f.in_user.name, "user_aa", "in_user is the sender");
    assert_eq!(f.out_user.name, "user_bb", "out_user is the recipient");
}

#[tokio::test]
async fn remove_friend_removes_from_list() {
    let db = common::setup_db().await;
    let user_t_id = common::create_test_user(&db, "user_t", "user_t@test.com").await;
    let user_u_id = common::create_test_user(&db, "user_u", "user_u@test.com").await;

    let friendship = add_friend(user_t_id.to_raw(), "user_u".to_string(), &db).await.unwrap();
    let friendship_id = friendship.id.unwrap().to_raw();
    update_friend_request(user_u_id.to_raw(), friendship_id.clone(), &db, "accept").await.unwrap();

    let before = list_friends(user_t_id.to_raw(), &db).await.unwrap();
    assert_eq!(before.len(), 1);

    remove_friend(user_t_id.to_raw(), friendship_id, &db).await.expect("remove should succeed");

    let after = list_friends(user_t_id.to_raw(), &db).await.unwrap();
    assert_eq!(after.len(), 0);
}

#[tokio::test]
async fn remove_friend_by_uninvolved_user_fails() {
    let db = common::setup_db().await;
    let user_v_id = common::create_test_user(&db, "user_v", "user_v@test.com").await;
    let user_w_id = common::create_test_user(&db, "user_w", "user_w@test.com").await;
    let user_x_id = common::create_test_user(&db, "user_x", "user_x@test.com").await;

    let friendship = add_friend(user_v_id.to_raw(), "user_w".to_string(), &db).await.unwrap();
    let friendship_id = friendship.id.unwrap().to_raw();
    update_friend_request(user_w_id.to_raw(), friendship_id.clone(), &db, "accept").await.unwrap();

    let result = remove_friend(user_x_id.to_raw(), friendship_id, &db).await;
    assert!(result.is_err());
    let (status, _) = result.unwrap_err();
    assert_eq!(status, rocket::http::Status::Forbidden);
}

#[tokio::test]
async fn reject_friend_request_does_not_appear_in_list() {
    let db = common::setup_db().await;
    let user_h_id = common::create_test_user(&db, "user_h", "user_h@test.com").await;
    let user_i_id = common::create_test_user(&db, "user_i", "user_i@test.com").await;

    add_friend(user_h_id.to_raw(), "user_i".to_string(), &db).await.unwrap();

    let friendships: Vec<Friendship> = db
        .query("SELECT * FROM friendship WHERE status = 'pending'")
        .await
        .unwrap()
        .take(0)
        .unwrap();

    let friendship_id = friendships[0].id.as_ref().unwrap().to_raw();

    update_friend_request(user_i_id.to_raw(), friendship_id, &db, "reject")
        .await
        .expect("reject should succeed");

    let accepted = list_friends(user_h_id.to_raw(), &db).await.unwrap();
    assert_eq!(accepted.len(), 0);
}

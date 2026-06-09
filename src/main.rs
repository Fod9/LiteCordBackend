#[macro_use]
extern crate rocket;
use std::sync::Arc;

use litecord_backend::cdn::{ensure_bucket, init_s3};
use litecord_backend::chat::hub::ChatHub;
use litecord_backend::db::init_db;
use litecord_backend::routes::*;

#[get("/")]
fn index() -> &'static str {
    "Hello, world!"
}

#[launch]
async fn rocket() -> _ {
    dotenvy::dotenv().ok();

    let db = init_db().await.expect("Failed to initialize database");
    let s3 = init_s3();
    ensure_bucket(&s3).await;

    rocket::build()
        .manage(db)
        .manage(s3)
        .manage(Arc::new(ChatHub::new()))
        .mount("/", routes![index,])
        .mount("/cdn", routes![cdn_routes::presign_route,])
        .mount("/ws", routes![websockets_routes::websocket_index])
        .mount(
            "/auth",
            routes![
                auth_routes::signup_route,
                auth_routes::login_route,
                auth_routes::refresh_route,
                auth_routes::get_my_info_route,
            ],
        )
        .mount(
            "/channels",
            routes![
                channels_routes::list_dm_channels,
                channels_routes::create_dm_channel_route,
                messages_routes::get_messages,
            ],
        )
        .mount(
            "/friends",
            routes![
                friends_routes::add_friend_route,
                friends_routes::remove_friend_route,
                friends_routes::list_friends_route,
                friends_routes::list_pending_requests_route,
                friends_routes::update_friend_request_route
            ],
        )
        .mount(
            "/guilds",
            routes![
                guilds_routes::create_guild_route,
                guilds_routes::list_guilds_route,
                guilds_routes::delete_guild_route,
                guilds_routes::leave_guild_route,
                guilds_routes::create_invite_route,
                guilds_routes::join_guild_route,
                guilds_routes::list_guild_members_route,
                guilds_routes::kick_member_route,
                guilds_routes::update_guild_route,
                guilds_routes::list_guild_invites_route,
                guilds_routes::revoke_invite_route,
                guild_channels_routes::create_channel_route,
                guild_channels_routes::list_channels_route,
                guild_channels_routes::delete_channel_route,
                roles_routes::create_role_route,
                roles_routes::list_roles_route,
                roles_routes::delete_role_route,
                roles_routes::assign_role_route,
                roles_routes::remove_role_route,
            ],
        )
}

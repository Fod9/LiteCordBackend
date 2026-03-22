#[macro_use]
extern crate rocket;
use std::sync::Arc;

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

    rocket::build()
        .manage(db)
        .manage(Arc::new(ChatHub::new()))
        .mount("/", routes![index,])
        .mount("/ws", routes![websockets_routes::websocket_index])
        .mount(
            "/auth",
            routes![
                auth_routes::signup_route,
                auth_routes::login_route,
                auth_routes::refresh_route,
            ],
        )
}

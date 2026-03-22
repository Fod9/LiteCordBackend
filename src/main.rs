#[macro_use]
extern crate rocket;
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

    rocket::build().manage(db).mount(
        "/",
        routes![index, signup_route, login_route, refresh_route],
    )
}

#[macro_use]
extern crate rocket;
use litecord_backend::db::init_db;
use litecord_backend::environment::Config;

use rocket::figment::{Figment, providers::Env};

#[get("/")]
fn index() -> &'static str {
    "Hello, world!"
}

#[launch]
async fn rocket() -> _ {
    dotenvy::dotenv().ok();
    let config: Config = Figment::new()
        .merge(Env::prefixed("ROCKET_"))
        .extract()
        .expect("Erreur de config");

    init_db(config)
        .await
        .expect("Failed to initialize database");

    rocket::build().mount("/", routes![index])
}

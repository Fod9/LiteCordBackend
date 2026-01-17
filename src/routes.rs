use crate::hashing::hash_password;
use crate::models::db::User;
use crate::models::user::CreateUser;
use chrono::prelude::*;
use rocket::{State, serde::json::Json};
use rocket::{get, post};
use surrealdb::Surreal;
use surrealdb::engine::remote::ws::Client;
use surrealdb::sql::Datetime;

#[post("/auth/signup", format = "json", data = "<user>")]
pub async fn signup(
    user: Json<CreateUser>,
    db: &State<Surreal<Client>>,
) -> Result<Json<User>, String> {
    if let Ok(hashed_password) = hash_password(&user.password) {
        let content = User {
            id: None,
            name: user.name.clone(),
            password: hashed_password,
            email: user.email.clone(),
            display_name: user.name.clone(),
            profile_picture: String::from(""),
            status: crate::models::db::ActivityStatus::Online,
            created_at: Datetime::from(Utc::now()),
        };

        let created_record: Option<User> = db
            .create("user")
            .content(content)
            .await
            .map_err(|e| format!("Erreur DB: {}", e))?;

        match created_record {
            Some(u) => Ok(Json(u)),
            None => Err("Erreur lors de la création".to_string()),
        }
    } else {
        Err("Erreur de hashage du mot de passe".to_string())
    }
}

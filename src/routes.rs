use crate::hashing::hash_password;
use crate::jwt;
use crate::models::db::User;
use crate::models::user::{CreateUser, LoginSuccess, LoginUser};
use ::jwt::Token;
use chrono::prelude::*;
use rocket::http::Status;
use rocket::post;
use rocket::{State, serde::json::Json};
use surrealdb::Surreal;
use surrealdb::engine::remote::ws::Client;
use surrealdb::sql::Datetime;

#[post("/auth/signup", format = "json", data = "<user_json>")]
pub async fn signup(
    user_json: Json<CreateUser>,
    db: &State<Surreal<Client>>,
) -> Result<(Status, Json<LoginSuccess>), (Status, String)> {
    let user = user_json.into_inner();

    let mut result = db
        .query("SELECT * FROM user WHERE email = $email OR name = $name")
        .bind(("email", user.email.clone()))
        .bind(("name", user.name.clone()))
        .await
        .map_err(|e| (Status::InternalServerError, e.to_string()))?;

    let existing_users: Vec<User> = result
        .take(0)
        .map_err(|e| (Status::InternalServerError, e.to_string()))?;

    if !existing_users.is_empty() {
        return Err((
            Status::Conflict,
            "Email ou Nom d'utilisateur déjà utilisé".to_string(),
        ));
    }

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
            .map_err(|e| (Status::InternalServerError, e.to_string()))?;

        match created_record {
            Some(u) => {
                let user_id = u.id.ok_or((
                    Status::InternalServerError,
                    "Erreur aucun ID trouvé pour cet utilisateur".to_string(),
                ))?;

                let token = jwt::generate_jwt(&user_id)
                    .map_err(|_| {
                        (
                            Status::InternalServerError,
                            "Erreur lors de la création du token".to_string(),
                        )
                    })?
                    .as_str()
                    .to_string();

                let refresh_token = jwt::generate_jwt(&user_id)
                    .map_err(|_| {
                        (
                            Status::InternalServerError,
                            "Erreur lors de la création du refresh token".to_string(),
                        )
                    })?
                    .as_str()
                    .to_string();

                let login_success = LoginSuccess {
                    token: token,
                    refresh_token: refresh_token,
                };

                Ok((Status::Created, Json(login_success)))
            }
            None => Err((
                Status::InternalServerError,
                "Erreur lors de la création".to_string(),
            )),
        }
    } else {
        Err((
            Status::InternalServerError,
            "Erreur de hashage du mot de passe".to_string(),
        ))
    }
}

#[post("/auth/login", format = "json", data = "<user_json>")]
pub async fn login(
    user_json: Json<LoginUser>,
    db: &State<Surreal<Client>>,
) -> Result<(Status, Json<User>), (Status, String)> {
    let user = user_json.into_inner();

    let user_with_email: Option<User> = db
        .query("SELECT * FROM user WHERE email = $email")
        .bind(("email", user.email.clone()))
        .await
        .map_err(|e| (Status::InternalServerError, e.to_string()))?
        .take(0)
        .map_err(|e| (Status::InternalServerError, e.to_string()))?;

    if let Some(db_user) = user_with_email {
        match crate::hashing::verify_password(&user.password, &db_user.password) {
            Ok(true) => Ok((Status::Ok, Json(db_user))),
            Ok(false) => Err((Status::Unauthorized, "Identifiants invalides".to_string())),
            Err(e) => Err((Status::InternalServerError, e.to_string())),
        }
    } else {
        Err((Status::Unauthorized, "Identifiants invalides".to_string()))
    }
}

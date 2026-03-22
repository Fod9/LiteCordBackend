use crate::hashing::hash_password;
use crate::jwt;
use crate::models::db::User;
use crate::models::user::{CreateUser, LoginSuccess, LoginUser};
use chrono::prelude::*;
use rocket::http::Status;
use rocket::{State, serde::json::Json};
use surrealdb::Surreal;
use surrealdb::engine::remote::ws::Client;
use surrealdb::sql::Datetime;
use surrealdb::sql::Thing;

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

pub async fn signin(
    user_json: Json<LoginUser>,
    db: &State<Surreal<Client>>,
) -> Result<(Status, Json<LoginSuccess>), (Status, String)> {
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
            Ok(true) => {
                let user_id = &db_user.id.ok_or((
                    Status::InternalServerError,
                    "No id found for this user".to_string(),
                ))?;

                let token = jwt::generate_jwt(&user_id)
                    .map_err(|_| {
                        (
                            Status::InternalServerError,
                            "Cannot generate a token".to_string(),
                        )
                    })?
                    .as_str()
                    .to_string();

                let refresh_token = jwt::generate_jwt(&user_id).map_err(|_| {
                    (
                        Status::InternalServerError,
                        "Cannot generate a token".to_string(),
                    )
                })?;

                println!(
                    "Storing JWT {} in DB for user_id {}",
                    refresh_token.as_str(),
                    user_id
                );
                jwt::store_refresh_token_in_db(refresh_token.as_str(), user_id, db)
                    .await
                    .map_err(|e| {
                        (
                            Status::InternalServerError,
                            format!("Cannot store refresh token in db: {}", e),
                        )
                    })?;

                Ok((
                    Status::Ok,
                    Json(LoginSuccess {
                        token: token,
                        refresh_token: refresh_token.as_str().to_string(),
                    }),
                ))
            }
            Ok(false) => Err((Status::Unauthorized, "Identifiants invalides".to_string())),
            Err(e) => Err((Status::InternalServerError, e.to_string())),
        }
    } else {
        Err((Status::Unauthorized, "Identifiants invalides".to_string()))
    }
}

pub async fn refresh_token(
    refresh_token: String,
    db: &State<Surreal<Client>>,
) -> Result<(Status, Json<LoginSuccess>), (Status, String)> {
    let token_data = jwt::decode_token(&refresh_token).map_err(|_| {
        (
            Status::Unauthorized,
            "Token de rafraîchissement invalide".to_string(),
        )
    })?;
    let user_id_str = token_data
        .get("user_id")
        .ok_or((
            Status::Unauthorized,
            "Token de rafraîchissement invalide : user_id manquant".to_string(),
        ))?
        .clone();
    let user_id: Thing = user_id_str
        .parse()
        .map_err(|_| (Status::Unauthorized, "user_id malformé".to_string()))?;

    let token_in_db = jwt::check_if_refresh_token_in_db(refresh_token.clone(), &user_id, db).await;
    if !token_in_db {
        return Err((
            Status::Unauthorized,
            "Token de rafraîchissement non trouvé ou invalide".to_string(),
        ));
    }

    let new_token_str = jwt::generate_jwt(&user_id)
        .map(|t| t.as_str().to_string())
        .map_err(|_| {
            (
                Status::InternalServerError,
                "Erreur lors de la génération du token".to_string(),
            )
        })?;

    let new_refresh_token_str = jwt::generate_jwt(&user_id)
        .map(|t| t.as_str().to_string())
        .map_err(|_| {
            (
                Status::InternalServerError,
                "Erreur lors de la génération du refresh token".to_string(),
            )
        })?;

    jwt::delete_refresh_token_from_db(&refresh_token, &user_id, db)
        .await
        .map_err(|_| {
            (
                Status::InternalServerError,
                "Erreur lors de la suppression du token".to_string(),
            )
        })?;

    jwt::store_refresh_token_in_db(&new_refresh_token_str, &user_id, db)
        .await
        .map_err(|_| {
            (
                Status::InternalServerError,
                "Erreur lors du stockage du refresh token".to_string(),
            )
        })?;

    Ok((
        Status::Ok,
        Json(LoginSuccess {
            token: new_token_str,
            refresh_token: new_refresh_token_str,
        }),
    ))
}

use crate::models::user::{CreateUser, LoginSuccess, LoginUser, RefreshTokenRequest};
use crate::users::auth;
use rocket::http::Status;
use rocket::post;
use rocket::{State, serde::json::Json};
use surrealdb::Surreal;
use surrealdb::engine::remote::ws::Client;

#[post("/auth/signup", format = "json", data = "<user_json>")]
pub async fn signup_route(
    user_json: Json<CreateUser>,
    db: &State<Surreal<Client>>,
) -> Result<(Status, Json<LoginSuccess>), (Status, String)> {
    auth::signup(user_json, db).await
}

#[post("/auth/login", format = "json", data = "<user_json>")]
pub async fn login_route(
    user_json: Json<LoginUser>,
    db: &State<Surreal<Client>>,
) -> Result<(Status, Json<LoginSuccess>), (Status, String)> {
    auth::signin(user_json, db).await
}

#[post("/auth/refresh", format = "json", data = "<refresh_token>")]
pub async fn refresh_route(
    refresh_token: Json<RefreshTokenRequest>,
    db: &State<Surreal<Client>>,
) -> Result<(Status, Json<LoginSuccess>), (Status, String)> {
    auth::refresh_token(refresh_token.into_inner().refresh_token, db).await
}

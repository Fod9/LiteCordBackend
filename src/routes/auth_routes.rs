use crate::models::user::{
    AuthenticatedUser, CreateUser, LoginSuccess, LoginUser, RefreshTokenRequest,
};
use crate::users::auth;
use rocket::get;
use rocket::http::Status;
use rocket::post;
use rocket::{State, serde::json::Json};
use surrealdb::Surreal;
use surrealdb::engine::any::Any;

#[post("/signup", format = "json", data = "<user_json>")]
pub async fn signup_route(
    user_json: Json<CreateUser>,
    db: &State<Surreal<Any>>,
) -> Result<(Status, Json<LoginSuccess>), (Status, String)> {
    auth::signup(user_json, db).await
}

#[post("/login", format = "json", data = "<user_json>")]
pub async fn login_route(
    user_json: Json<LoginUser>,
    db: &State<Surreal<Any>>,
) -> Result<(Status, Json<LoginSuccess>), (Status, String)> {
    auth::signin(user_json, db).await
}

#[post("/refresh", format = "json", data = "<refresh_token>")]
pub async fn refresh_route(
    refresh_token: Json<RefreshTokenRequest>,
    db: &State<Surreal<Any>>,
) -> Result<(Status, Json<LoginSuccess>), (Status, String)> {
    auth::refresh_token(refresh_token.into_inner().refresh_token, db).await
}

#[get("/me")]
pub async fn get_my_info_route(
    token: AuthenticatedUser,
    db: &State<Surreal<Any>>,
) -> Result<(Status, String), (Status, String)> {
    auth::get_my_info(token.token.clone(), db).await
}

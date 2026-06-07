use crate::jwt::decode_token;
use rocket::Request;
use rocket::http::Status;
use rocket::request::{self, FromRequest, Outcome};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
#[serde(crate = "rocket::serde")]
pub struct CreateUser {
    pub name: String,
    pub password: String,
    pub email: String,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(crate = "rocket::serde")]
pub struct LoginUser {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct LoginSuccess {
    pub token: String,
    pub refresh_token: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct RefreshTokenRequest {
    pub refresh_token: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct CreateDmChannelRequest {
    pub recipient_ids: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct AuthenticatedUser {
    pub user_id: String,
    pub token: String,
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for AuthenticatedUser {
    type Error = ();

    async fn from_request(request: &'r Request<'_>) -> request::Outcome<Self, Self::Error> {
        match request.headers().get_one("authorization") {
            Some(header) => {
                let token = header.strip_prefix("Bearer ").unwrap_or(header);
                if let Ok(claims) = decode_token(token) {
                    Outcome::Success(AuthenticatedUser {
                        user_id: claims.user_id,
                        token: token.to_string(),
                    })
                } else {
                    return Outcome::Forward(Status::Unauthorized);
                }
            }
            None => Outcome::Forward(Status::Unauthorized),
        }
    }
}

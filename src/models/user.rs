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

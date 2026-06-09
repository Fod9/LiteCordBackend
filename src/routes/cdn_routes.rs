use crate::cdn::presign_upload;
use crate::models::user::AuthenticatedUser;
use aws_sdk_s3::Client as S3Client;
use rocket::http::Status;
use rocket::serde::json::Json;
use rocket::{State, post};
use serde::{Deserialize, Serialize};

const MAX_SIZE_BYTES: u64 = 25 * 1024 * 1024;

#[derive(Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct PresignRequest {
    pub filename: String,
    pub content_type: String,
    pub size: u64,
}

#[derive(Serialize)]
pub struct PresignResponse {
    pub upload_url: String,
    pub cdn_url: String,
}

#[post("/presign", format = "json", data = "<body>")]
pub async fn presign_route(
    _token: AuthenticatedUser,
    body: Json<PresignRequest>,
    s3: &State<S3Client>,
) -> Result<(Status, String), (Status, String)> {
    let body = body.into_inner();

    if body.size > MAX_SIZE_BYTES {
        return Err((Status::PayloadTooLarge, "File exceeds 25 MB limit".to_string()));
    }

    if body.filename.is_empty() || body.content_type.is_empty() {
        return Err((Status::BadRequest, "filename and content_type are required".to_string()));
    }

    let result = presign_upload(s3, &body.filename)
        .await
        .map_err(|e| (Status::InternalServerError, e))?;

    let response = serde_json::to_string(&PresignResponse {
        upload_url: result.upload_url,
        cdn_url: result.cdn_url,
    })
    .map_err(|e| (Status::InternalServerError, e.to_string()))?;

    Ok((Status::Ok, response))
}

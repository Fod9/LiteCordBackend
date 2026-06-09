use aws_sdk_s3::Client as S3Client;
use aws_sdk_s3::config::{BehaviorVersion, Builder, Credentials, Region};
use aws_sdk_s3::presigning::PresigningConfig;
use std::time::Duration;
use uuid::Uuid;

use crate::environment::get_config;

pub async fn ensure_bucket(s3: &S3Client) {
    let config = get_config();
    let Some(bucket) = config.s3_bucket.as_deref() else { return; };

    match s3.head_bucket().bucket(bucket).send().await {
        Ok(_) => println!("Bucket '{bucket}' already exists."),
        Err(head_err) => {
            eprintln!("head_bucket error: {head_err:?}");
            match s3.create_bucket().bucket(bucket).send().await {
                Ok(_) => println!("Bucket '{bucket}' created."),
                Err(create_err) => eprintln!("create_bucket error: {create_err:?}"),
            }
        }
    }
}

pub fn init_s3() -> S3Client {
    let config = get_config();
    let access_key = config.s3_access_key.as_deref().unwrap_or("");
    let secret_key = config.s3_secret_key.as_deref().unwrap_or("");
    let endpoint = config.s3_endpoint.as_deref().unwrap_or("http://localhost:9000");

    let creds = Credentials::new(access_key, secret_key, None, None, "litecord");
    let s3_config = Builder::new()
        .endpoint_url(endpoint)
        .credentials_provider(creds)
        .region(Region::new("us-east-1"))
        .force_path_style(true)
        .behavior_version(BehaviorVersion::latest())
        .build();
    S3Client::from_conf(s3_config)
}

pub struct PresignResult {
    pub upload_url: String,
    pub cdn_url: String,
}

fn build_client(endpoint: &str, access_key: &str, secret_key: &str) -> S3Client {
    let creds = Credentials::new(access_key, secret_key, None, None, "litecord");
    let s3_config = Builder::new()
        .endpoint_url(endpoint)
        .credentials_provider(creds)
        .region(Region::new("us-east-1"))
        .force_path_style(true)
        .behavior_version(BehaviorVersion::latest())
        .build();
    S3Client::from_conf(s3_config)
}

pub async fn presign_upload(
    s3: &S3Client,
    filename: &str,
) -> Result<PresignResult, String> {
    let config = get_config();
    let bucket = config.s3_bucket.as_deref()
        .ok_or("ROCKET_S3_BUCKET is not configured")?;
    let cdn_base = config.cdn_base_url.as_deref()
        .ok_or("ROCKET_CDN_BASE_URL is not configured")?;
    let access_key = config.s3_access_key.as_deref().unwrap_or("");
    let secret_key = config.s3_secret_key.as_deref().unwrap_or("");

    let key = format!("{}/{}", Uuid::new_v4(), filename);

    let presigning_config = PresigningConfig::expires_in(Duration::from_secs(300))
        .map_err(|e| e.to_string())?;

    // Use public endpoint for presigning so the Host header in the signature
    // matches what the client will send. Fall back to the main client if no
    // public endpoint is configured.
    let presign_client;
    let client_ref = if let Some(public_endpoint) = config.s3_public_endpoint.as_deref() {
        presign_client = build_client(public_endpoint, access_key, secret_key);
        &presign_client
    } else {
        s3
    };

    let presigned = client_ref
        .put_object()
        .bucket(bucket)
        .key(&key)
        .presigned(presigning_config)
        .await
        .map_err(|e| e.to_string())?;

    let cdn_url = format!("{}/{}", cdn_base.trim_end_matches('/'), key);

    Ok(PresignResult {
        upload_url: presigned.uri().to_string(),
        cdn_url,
    })
}

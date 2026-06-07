use crate::environment::get_config;
use crate::models::db::RefreshToken;
use aes_gcm::AeadCore;
use aes_gcm::aead::{Aead, KeyInit, OsRng};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use hmac::{Hmac, Mac};
use jwt::VerifyWithKey;
use jwt::{AlgorithmType, Header, SignWithKey, Token, token::Signed};

use sha2::Sha384;
use std::collections::BTreeMap;
use surrealdb::{Surreal, engine::any::Any, sql::Thing};

pub type SignedToken = Token<Header, BTreeMap<String, String>, Signed>;

pub struct TokenClaims {
    pub iat: String,
    pub user_id: String,
    pub token_type: String,
}

impl TokenClaims {
    pub fn from_claims(
        claims: &BTreeMap<String, String>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let iat = claims.get("iat").ok_or("Missing 'iat' claim")?.to_string();
        let user_id = claims
            .get("user_id")
            .ok_or("Missing 'user_id' claim")?
            .to_string();
        let token_type = claims
            .get("type")
            .ok_or("Missing 'type' claim")?
            .to_string();
        Ok(TokenClaims {
            iat,
            user_id,
            token_type,
        })
    }
}

pub fn generate_jwt(
    id: &Thing,
    token_type: String,
) -> Result<SignedToken, Box<dyn std::error::Error>> {
    let config = get_config();
    let key: Hmac<Sha384> = <Hmac<Sha384> as Mac>::new_from_slice(config.jwt_secret.as_bytes())?;
    let mut claims: BTreeMap<String, String> = BTreeMap::new();
    claims.insert(
        "iat".to_string(),
        format!("{}", chrono::Utc::now().timestamp()),
    );
    claims.insert("type".to_string(), token_type);
    claims.insert("user_id".to_string(), id.to_string());

    let header = Header {
        algorithm: AlgorithmType::Hs384,
        ..Default::default()
    };
    let token = Token::new(header, claims).sign_with_key(&key)?;
    Ok(token)
}

pub fn decode_token(token: &str) -> Result<TokenClaims, Box<dyn std::error::Error>> {
    let config = get_config();
    let key: Hmac<Sha384> = <Hmac<Sha384> as Mac>::new_from_slice(config.jwt_secret.as_bytes())?;
    let token: Token<Header, BTreeMap<String, String>, _> = token.verify_with_key(&key)?;

    let result = token
        .claims()
        .get("iat")
        .ok_or("Token missing 'iat' claim")?;
    if let Ok(iat) = result.parse::<i64>() {
        let now = chrono::Utc::now().timestamp();
        match token.claims().get("type") {
            Some(token_type) if token_type == "access" => {
                if now - iat > config.token_expiration_seconds {
                    return Err("Token expired".into());
                }
            }
            Some(token_type) if token_type == "refresh" => {
                if now - iat > config.refresh_token_expiration_seconds {
                    return Err("Refresh token expired".into());
                }
            }
            _ => return Err("Invalid 'type' claim".into()),
        }
    } else {
        return Err("Invalid 'iat' claim".into());
    }

    Ok(TokenClaims::from_claims(token.claims())?)
}

pub async fn check_if_refresh_token_in_db(
    jwt: String,
    user_id: &Thing,
    db: &Surreal<Any>,
) -> bool {
    let result = db
        .query("SELECT * FROM RefreshToken WHERE user = $user")
        .bind(("user", user_id.clone()))
        .await;

    if let Ok(mut res) = result {
        let tokens: Vec<RefreshToken> = res.take(0).unwrap_or_default();
        for token in tokens {
            if let Ok(decrypted_token) = decrypt_aes_refresh_token(&token.token) {
                if decrypted_token == jwt {
                    return true; // <-- le fix principal
                }
            }
        }
    }
    false
}

pub async fn store_refresh_token_in_db(
    jwt: &str,
    user_id: &Thing,
    db: &Surreal<Any>,
) -> Result<(), Box<dyn std::error::Error>> {
    let encrypted_token = encrypt_aes_refresh_token(jwt)?;
    db.query("CREATE RefreshToken SET token = $token_str, user = $user")
        .bind(("token_str", encrypted_token))
        .bind(("user", user_id.clone()))
        .await?;
    Ok(())
}

pub async fn delete_refresh_token_from_db(
    jwt: &str,
    user_id: &Thing,
    db: &Surreal<Any>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let result = db
        .query("SELECT * FROM RefreshToken WHERE user = $user")
        .bind(("user", user_id.clone()))
        .await;

    if let Ok(mut res) = result {
        let tokens: Vec<RefreshToken> = res.take(0).unwrap_or_default();
        let mut target_id = None;
        for token in tokens {
            if let Ok(decrypted) = decrypt_aes_refresh_token(&token.token) {
                if decrypted == jwt {
                    target_id = Some(token.id);
                    break;
                }
            }
        }
        if let Some(id) = target_id {
            db.query("DELETE $record_id")
                .bind(("record_id", id))
                .await?;
            return Ok(());
        }
    }
    Err("Refresh token not found".into())
}

pub fn encrypt_aes_refresh_token(jwt: &str) -> Result<String, Box<dyn std::error::Error>> {
    let config = get_config();
    let cipher = aes_gcm::Aes256Gcm::new_from_slice(config.aes_key.as_bytes())?;
    let nonce = aes_gcm::Aes256Gcm::generate_nonce(&mut OsRng);
    let ciphertext = cipher
        .encrypt(&nonce, jwt.as_bytes())
        .map_err(|e| format!("Encryption failed : {e}"))?;

    let mut combined = nonce.to_vec();
    combined.extend_from_slice(&ciphertext);
    Ok(BASE64.encode(combined))
}

pub fn decrypt_aes_refresh_token(
    encrypted_jwt: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let config = get_config();
    let cipher = aes_gcm::Aes256Gcm::new_from_slice(config.aes_key.as_bytes())?;
    let combined = BASE64.decode(encrypted_jwt)?;

    if combined.len() < 12 {
        return Err("Invalid encrypted token".into());
    }

    let (nonce_bytes, ciphertext) = combined.split_at(12);
    let nonce = aes_gcm::Nonce::from_slice(nonce_bytes);
    let decrypted_data = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| format!("Decryption failed {e}"))?;
    Ok(String::from_utf8(decrypted_data)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use surrealdb::sql::Thing;

    fn setup_env() {
        unsafe {
            std::env::set_var("ROCKET_JWT_SECRET", "test_secret_key_for_litecord_tests_only");
            std::env::set_var("ROCKET_AES_KEY", "litecord_test_aes_key_32_bytes!!");
            std::env::set_var("ROCKET_TOKEN_EXPIRATION_SECONDS", "3600");
            std::env::set_var("ROCKET_REFRESH_TOKEN_EXPIRATION_SECONDS", "604800");
            std::env::set_var("ROCKET_DB_URL", "localhost:8000");
            std::env::set_var("ROCKET_DB_USER", "root");
            std::env::set_var("ROCKET_DB_PASSWORD", "root");
            std::env::set_var("ROCKET_DB_CONFIG_FILE", "db.surql");
        }
    }

    fn test_user_thing() -> Thing {
        surrealdb::sql::thing("user:test123").unwrap()
    }

    #[test]
    fn generate_and_decode_access_token() {
        setup_env();
        let user_id = test_user_thing();
        let token = generate_jwt(&user_id, "access".to_string()).unwrap();
        let claims = decode_token(token.as_str()).unwrap();
        assert_eq!(claims.token_type, "access");
        assert!(claims.user_id.contains("test123"));
    }

    #[test]
    fn generate_and_decode_refresh_token() {
        setup_env();
        let user_id = test_user_thing();
        let token = generate_jwt(&user_id, "refresh".to_string()).unwrap();
        let claims = decode_token(token.as_str()).unwrap();
        assert_eq!(claims.token_type, "refresh");
    }

    #[test]
    fn decode_tampered_token_fails() {
        setup_env();
        let result = decode_token("this.is.not.a.valid.jwt");
        assert!(result.is_err());
    }

    #[test]
    fn encrypt_then_decrypt_roundtrip() {
        setup_env();
        let original = "some.jwt.token.string";
        let encrypted = encrypt_aes_refresh_token(original).unwrap();
        let decrypted = decrypt_aes_refresh_token(&encrypted).unwrap();
        assert_eq!(decrypted, original);
    }

    #[test]
    fn two_encryptions_of_same_value_differ() {
        setup_env();
        let enc_a = encrypt_aes_refresh_token("same_token").unwrap();
        let enc_b = encrypt_aes_refresh_token("same_token").unwrap();
        assert_ne!(enc_a, enc_b, "nonces must make ciphertexts unique");
    }
}

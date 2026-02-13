use std::collections::BTreeMap;

use crate::environment::get_config;
use hmac::{Hmac, Mac};
use jwt::{AlgorithmType, Header, SignWithKey, Token, token::Signed};
use sha2::Sha384;
use surrealdb::sql::Thing;

pub type SignedToken = Token<Header, BTreeMap<String, String>, Signed>;

pub fn generate_jwt(id: &Thing) -> Result<SignedToken, Box<dyn std::error::Error>> {
    let config = get_config();
    let key: Hmac<Sha384> = Hmac::new_from_slice(config.jwt_secret.as_bytes())?;
    let mut claims: BTreeMap<String, String> = BTreeMap::new();
    claims.insert(
        "iat".to_string(),
        format!("{}", chrono::Utc::now().timestamp()),
    );
    claims.insert("user_id".to_string(), id.to_string());

    let header = Header {
        algorithm: AlgorithmType::Hs384,
        ..Default::default()
    };

    let token = Token::new(header, claims).sign_with_key(&key)?;
    Ok(token)
}

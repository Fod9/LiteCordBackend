pub mod cdn;
pub mod chat;
pub mod channels;
pub mod error;
pub mod friends;
pub mod guild_channels;
pub mod guilds;
pub mod hashing;
pub mod jwt;
pub mod messages;
pub mod models;
pub mod permissions;
pub mod roles;
pub mod routes;
pub mod users;

pub mod environment {
    use rocket::figment::{Figment, providers::Env};
    use serde::Deserialize;
    use std::sync::OnceLock;

    #[derive(Debug, Deserialize)]
    pub struct Config {
        pub db_url: String,
        pub db_user: String,
        pub db_password: String,
        pub db_config_file: String,
        pub jwt_secret: String,
        pub aes_key: String,
        pub token_expiration_seconds: i64,
        pub refresh_token_expiration_seconds: i64,
        pub s3_endpoint: Option<String>,
        pub s3_public_endpoint: Option<String>,
        pub s3_bucket: Option<String>,
        pub s3_access_key: Option<String>,
        pub s3_secret_key: Option<String>,
        pub cdn_base_url: Option<String>,
    }

    static CONFIG: OnceLock<Config> = OnceLock::new();

    pub fn get_config() -> &'static Config {
        CONFIG.get_or_init(|| {
            Figment::new()
                .merge(Env::prefixed("ROCKET_"))
                .extract()
                .expect("Erreur lors du chargement de la configuration")
        })
    }
}

pub mod db {
    use crate::environment::get_config;
    use std::fs;
    use surrealdb::{
        Surreal,
        engine::any::Any,
        opt::auth::Root,
    };

    pub async fn init_db() -> Result<Surreal<Any>, surrealdb::Error> {
        let config = get_config();
        let database_instance = surrealdb::engine::any::connect(format!("ws://{}", &config.db_url)).await?;

        database_instance
            .signin(Root {
                username: &config.db_user,
                password: &config.db_password,
            })
            .await?;

        database_instance
            .use_ns("litecord")
            .use_db("litecord")
            .await?;

        let schema =
            fs::read_to_string(&config.db_config_file).expect("Failed to read schema file");

        let mut response = database_instance.query(&schema).await?;
        let num_statements = response.num_statements();

        for i in 0..num_statements {
            if let Err(e) = response.take::<surrealdb::Value>(i) {
                eprintln!("Schema statement {} failed: {:?}", i, e);
            }
        }

        database_instance.query(&schema).await?;

        Ok(database_instance)
    }
}

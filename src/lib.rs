pub mod error;
pub mod hashing;
pub mod jwt;
pub mod models;
pub mod routes;

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
        engine::remote::ws::{Client, Ws},
        opt::auth::Root,
    };

    pub async fn init_db() -> Result<Surreal<Client>, surrealdb::Error> {
        let database_instance: Surreal<Client> = Surreal::init();
        let config = get_config();

        database_instance.connect::<Ws>(&config.db_url).await?;

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

        database_instance.query(&schema).await?;

        Ok(database_instance)
    }
}

pub mod error;
pub mod hashing;
pub mod models;
pub mod routes;

pub mod environment {
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    pub struct Config {
        pub db_url: String,
        pub db_user: String,
        pub db_password: String,
        pub db_config_file: String,
    }
}

pub mod db {
    use crate::environment::Config;
    use std::fs;
    use surrealdb::{
        Surreal,
        engine::remote::ws::{Client, Ws},
        opt::auth::Root,
    };

    pub async fn init_db(config: Config) -> Result<Surreal<Client>, surrealdb::Error> {
        let database_instance: Surreal<Client> = Surreal::init();

        database_instance.connect::<Ws>(config.db_url).await?;

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

        let schema = fs::read_to_string(config.db_config_file).expect("Failed to read schema file");

        database_instance.query(&schema).await?;

        Ok(database_instance)
    }
}

pub mod error;
pub mod models;

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
    use std::sync::LazyLock;
    use surrealdb::{
        Surreal,
        engine::remote::ws::{Client, Ws},
        opt::auth::Root,
    };

    static DB: LazyLock<Surreal<Client>> = LazyLock::new(Surreal::init);

    pub async fn init_db(config: Config) -> Result<(), surrealdb::Error> {
        DB.connect::<Ws>(config.db_url).await?;

        DB.signin(Root {
            username: &config.db_user,
            password: &config.db_password,
        })
        .await?;

        DB.use_ns("litecord").use_db("litecord").await?;

        let schema = fs::read_to_string(config.db_config_file).expect("Failed to read schema file");

        DB.query(&schema).await?;

        Ok(())
    }
}

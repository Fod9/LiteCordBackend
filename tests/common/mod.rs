use std::sync::OnceLock;
use surrealdb::Surreal;
use surrealdb::engine::any::Any;

static TEST_ENV: OnceLock<()> = OnceLock::new();

fn init_env() {
    TEST_ENV.get_or_init(|| {
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
    });
}

pub async fn setup_db() -> Surreal<Any> {
    init_env();

    let db = surrealdb::engine::any::connect("mem://").await.unwrap();

    let ns = format!(
        "test_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .subsec_nanos()
    );
    db.use_ns(&ns).use_db(&ns).await.unwrap();

    let schema = std::fs::read_to_string("db.surql").unwrap();
    let mut response = db.query(&schema).await.unwrap();
    let num = response.num_statements();
    for i in 0..num {
        let _ = response.take::<surrealdb::Value>(i);
    }

    db
}

pub async fn create_test_user(db: &Surreal<Any>, name: &str, email: &str) -> surrealdb::sql::Thing {
    use chrono::Utc;
    use litecord_backend::hashing::hash_password;
    use litecord_backend::models::db::{ActivityStatus, User};
    use surrealdb::sql::Datetime;

    let user = User {
        id: None,
        name: name.to_string(),
        display_name: name.to_string(),
        email: email.to_string(),
        password: hash_password("password123").unwrap(),
        profile_picture: String::new(),
        status: ActivityStatus::Online,
        created_at: Datetime::from(Utc::now()),
    };

    let created: Option<User> = db.create("user").content(user).await.unwrap();
    created.unwrap().id.unwrap()
}

pub async fn create_accepted_friendship(
    db: &Surreal<Any>,
    user_a: &surrealdb::sql::Thing,
    user_b: &surrealdb::sql::Thing,
) {
    db.query("RELATE $a->friendship->$b SET status = 'accepted'")
        .bind(("a", user_a.clone()))
        .bind(("b", user_b.clone()))
        .await
        .unwrap();
}

pub async fn create_test_dm_channel(db: &Surreal<Any>, user_id: &surrealdb::sql::Thing) -> String {
    use litecord_backend::models::db::DMChannel;

    let recipients_key = user_id.to_raw();

    let mut res = db
        .query("CREATE DMChannel SET recipients = [$user], owner = $user, recipients_key = $rk")
        .bind(("user", user_id.clone()))
        .bind(("rk", recipients_key))
        .await
        .unwrap();

    let channel: Option<DMChannel> = res.take(0).unwrap();
    channel.unwrap().id.unwrap().to_raw()
}

use r2d2_sqlite::SqliteConnectionManager;

pub type DatabasePool = r2d2::Pool<SqliteConnectionManager>;

pub fn create_pool(app_path: &str, key: &str) -> DatabasePool {
    let app_path = app_path.to_string();
    let key = key.to_string();
    let manager = SqliteConnectionManager::memory().with_init(move |connection| {
        connection.execute_batch(&format!(
            "
            ATTACH DATABASE '{app_path}/databases/KakaoTalk.db' AS db1;
            ATTACH DATABASE '{app_path}/databases/KakaoTalk2.db' AS db2;
            ATTACH DATABASE '{app_path}/databases/crypto_user_database' AS user KEY x'{key}';
            "
        ))
    });

    r2d2::Pool::builder()
        .max_size(10)
        .min_idle(Some(1))
        .build(manager)
        .expect("failed to create database pool")
}

pub fn query_current_user_id(pool: &DatabasePool) -> i64 {
    pool.get()
        .ok()
        .and_then(|connection| {
            connection
                .query_row(
                    "SELECT user_id FROM db1.chat_logs WHERE v LIKE '%isMine\":true%' LIMIT 1",
                    [],
                    |row| row.get(0),
                )
                .ok()
        })
        .unwrap_or_default()
}

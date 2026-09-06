mod channels;
mod error;
mod feed;
mod mapping;
mod members;
mod messages;
mod poller;
mod raw;
mod record;

pub use error::{DbError, DbResult};
pub use poller::start_poller;

pub type DatabasePool = r2d2::Pool<SqliteConnectionManager>;

use crate::crypto::crypto::Decryptor;
use r2d2::PooledConnection;
use r2d2_sqlite::SqliteConnectionManager;

const OPEN_CHANNEL_ID_MASK: i64 = 1 << 54;

fn placeholders(count: usize) -> String {
    vec!["?"; count].join(",")
}

#[derive(Clone)]
pub struct Database {
    pub pool: DatabasePool,
    pub current_user_id: i64,
    pub decryptor: Decryptor,
}

impl Database {
    pub fn open(app_path: &str, key: &str) -> DbResult<Self> {
        let app_path = app_path.to_owned();
        let key = key.to_owned();

        let manager =
            SqliteConnectionManager::memory().with_init(move |connection| {
                connection.execute_batch(&format!(
                    "
                    ATTACH DATABASE '{app_path}/databases/KakaoTalk.db' AS db1;
                    ATTACH DATABASE '{app_path}/databases/KakaoTalk2.db' AS db2;
                    ATTACH DATABASE '{app_path}/databases/crypto_user_database' AS user KEY x'{key}';
                    "
                ))
            });

        let pool = r2d2::Pool::builder()
            .max_size(10)
            .min_idle(Some(1))
            .build(manager)?;

        let current_user_id = {
            let connection = pool.get()?;

            connection.query_row(
                r#"SELECT user_id
                   FROM db1.chat_logs
                   WHERE v LIKE '%isMine":true%'
                   LIMIT 1"#,
                [],
                |row| row.get(0),
            )?
        };

        Ok(Self {
            pool,
            current_user_id,
            decryptor: Decryptor::new(current_user_id),
        })
    }

    fn connection(&self) -> DbResult<PooledConnection<SqliteConnectionManager>> {
        Ok(self.pool.get()?)
    }
}

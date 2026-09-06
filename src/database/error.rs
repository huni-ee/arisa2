use r2d2_sqlite::rusqlite;

#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("failed to get database connection: {0}")]
    Pool(#[from] r2d2::Error),

    #[error("database query failed: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("member not found: {0}")]
    MemberNotFound(i64),

    #[error("message not found: {0}")]
    MessageNotFound(i64),

    #[error("message mapping failed: {0}")]
    MessageMapping(String),
}

pub type DbResult<T> = Result<T, DbError>;

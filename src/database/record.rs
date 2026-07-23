#[derive(Clone, Debug)]
pub struct MessageRow {
    pub database_id: i64,
    pub message_id: i64,
    pub message_type: i32,
    pub channel_id: i64,
    pub user_id: i64,
    pub message: Option<String>,
    pub attachment: Option<String>,
    pub metadata: Option<String>,
    pub thread_id: Option<i64>,
    pub scope: i32,
}

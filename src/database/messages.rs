use std::collections::{HashMap, HashSet};

use r2d2_sqlite::rusqlite::{self, Row};

use crate::{database::record::MessageRow, proto::Event};

use super::{Database, DbError, DbResult, placeholders};

impl Database {
    pub fn latest_message_database_id(&self) -> DbResult<i64> {
        let connection = self.connection()?;

        Ok(connection.query_row(
            "SELECT COALESCE(MAX(_id), 0) FROM db1.chat_logs",
            [],
            |row| row.get(0),
        )?)
    }

    pub fn messages_after(&self, database_id: i64, limit: i64) -> DbResult<Vec<MessageRow>> {
        let connection = self.connection()?;

        let mut statement = connection.prepare(
            "SELECT _id, id, type, chat_id, user_id,
                    message, attachment, v, thread_id, scope
             FROM db1.chat_logs
             WHERE _id > ?1
             ORDER BY _id ASC
             LIMIT ?2",
        )?;

        let rows = statement.query_map([database_id, limit], read_message)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn get_message(&self, channel_id: i64, message_id: i64) -> DbResult<Event> {
        self.get_messages(channel_id, &[message_id])?
            .into_iter()
            .next()
            .ok_or(DbError::MessageNotFound(message_id))
    }

    pub fn get_messages(&self, channel_id: i64, message_ids: &[i64]) -> DbResult<Vec<Event>> {
        self.get_message_rows(channel_id, message_ids)?
            .into_iter()
            .map(|row| self.map_message(row))
            .collect()
    }

    fn get_message_rows(&self, channel_id: i64, message_ids: &[i64]) -> DbResult<Vec<MessageRow>> {
        if message_ids.is_empty() {
            return Ok(Vec::new());
        }

        let connection = self.connection()?;
        let unique_ids: HashSet<i64> = message_ids.iter().copied().collect();
        let unique_ids: Vec<i64> = unique_ids.into_iter().collect();
        let mut messages = HashMap::with_capacity(unique_ids.len());

        for chunk in unique_ids.chunks(500) {
            let sql = format!(
                "SELECT _id, id, type, chat_id, user_id,
                        message, attachment, v, thread_id, scope
                 FROM db1.chat_logs
                 WHERE chat_id = ? AND id IN ({})",
                placeholders(chunk.len()),
            );

            let mut params = Vec::with_capacity(chunk.len() + 1);
            params.push(channel_id);
            params.extend_from_slice(chunk);

            let mut statement = connection.prepare(&sql)?;
            let rows = statement.query_map(rusqlite::params_from_iter(params), read_message)?;

            for row in rows {
                let message = row?;
                messages.insert(message.message_id, message);
            }
        }

        message_ids
            .iter()
            .map(|message_id| {
                messages
                    .get(message_id)
                    .cloned()
                    .ok_or(DbError::MessageNotFound(*message_id))
            })
            .collect()
    }
}

fn read_message(row: &Row<'_>) -> rusqlite::Result<MessageRow> {
    Ok(MessageRow {
        database_id: row.get(0)?,
        message_id: row.get(1)?,
        message_type: row.get(2)?,
        channel_id: row.get(3)?,
        user_id: row.get(4)?,
        message: row.get(5)?,
        attachment: row.get(6)?,
        metadata: row.get(7)?,
        thread_id: row.get(8)?,
        scope: row.get(9)?,
    })
}

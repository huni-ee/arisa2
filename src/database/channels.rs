use serde_json::Value;

use crate::proto::{Channel, ChannelMembers};

use super::{Database, DbResult};

impl Database {
    pub fn get_channel(&self, channel_id: i64) -> DbResult<Channel> {
        let connection = self.connection()?;

        let (channel_type, private_meta, metadata): (String, Option<String>, Option<String>) =
            connection.query_row(
                "SELECT type, private_meta, v
                 FROM db1.chat_rooms WHERE id = ?1 LIMIT 1",
                [channel_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )?;

        let name = if channel_type == "OM" {
            Some(connection.query_row(
                "SELECT name FROM db2.open_link
                 WHERE id = (SELECT link_id FROM db1.chat_rooms WHERE id = ?1)
                 LIMIT 1",
                [channel_id],
                |row| row.get::<_, String>(0),
            )?)
        } else if let Some(user_ids) = metadata.as_deref().and_then(parse_display_user_ids) {
            let members = self.get_users_with_connection(&connection, channel_id, &user_ids)?;

            Some(
                members
                    .iter()
                    .map(|member| member.nickname.as_str())
                    .collect::<Vec<_>>()
                    .join(", "),
            )
        } else {
            None
        };

        Ok(Channel {
            id: channel_id,
            channel_type,
            name,
            private_name: private_meta.as_deref().and_then(parse_private_name),
        })
    }

    pub fn get_channel_members(&self, channel_id: i64) -> DbResult<ChannelMembers> {
        let connection = self.connection()?;

        let raw: Option<String> = connection.query_row(
            "SELECT active_member_ids
             FROM db1.chat_rooms WHERE id = ?1 LIMIT 1",
            [channel_id],
            |row| row.get(0),
        )?;

        let active_member_ids = raw
            .as_deref()
            .and_then(parse_i64_array)
            .unwrap_or_default();

        Ok(ChannelMembers {
            channel_id,
            active_member_ids,
        })
    }
}

fn parse_display_user_ids(raw: &str) -> Option<Vec<i64>> {
    let value: Value = serde_json::from_str(raw).ok()?;
    parse_i64_array_value(value.get("display_user_ids")?)
}

fn parse_i64_array(raw: &str) -> Option<Vec<i64>> {
    serde_json::from_str(raw).ok()
}

fn parse_i64_array_value(value: &Value) -> Option<Vec<i64>> {
    let ids: Vec<i64> = match value {
        Value::String(value) => value
            .split(',')
            .filter_map(|id| id.trim().parse().ok())
            .collect(),
        Value::Array(values) => values
            .iter()
            .filter_map(|value| {
                value
                    .as_i64()
                    .or_else(|| value.as_str().and_then(|value| value.parse().ok()))
            })
            .collect(),
        _ => return None,
    };

    (!ids.is_empty()).then_some(ids)
}

fn parse_private_name(raw: &str) -> Option<String> {
    let value: Value = serde_json::from_str(raw).ok()?;
    value.get("name")?.as_str().map(str::to_string)
}
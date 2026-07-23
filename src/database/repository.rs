use std::collections::{HashMap, HashSet};

use base64::{Engine, engine::general_purpose::STANDARD};
use r2d2_sqlite::rusqlite::{self, Row, types::ValueRef};
use serde_json::{Value, json};

use crate::proto::{
    Channel, ChannelMembers, Event, LinkMemberType, Member, OpenChannelMemberExtra, ProfileType,
};

use super::{DatabasePool, Decryptor, record::MessageRow};

const OPEN_CHANNEL_ID_MASK: i64 = 1 << 54;

#[derive(Clone)]
pub struct Database {
    pool: DatabasePool,
    current_user_id: i64,
    pub(super) decryptor: Decryptor,
}

impl Database {
    pub fn new(pool: DatabasePool, current_user_id: i64) -> Self {
        Self {
            pool,
            current_user_id,
            decryptor: Decryptor::new(current_user_id),
        }
    }

    pub fn get_user(&self, channel_id: i64, user_id: i64) -> Result<Member, String> {
        self.get_users(channel_id, &[user_id])?
            .into_iter()
            .next()
            .ok_or_else(|| format!("member not found: {user_id}"))
    }

    pub fn get_users(&self, channel_id: i64, user_ids: &[i64]) -> Result<Vec<Member>, String> {
        if user_ids.is_empty() {
            return Ok(Vec::new());
        }

        let connection = self.connection()?;
        let unique_ids: HashSet<i64> = user_ids.iter().copied().collect();
        let mut members = HashMap::new();
        let is_open_channel = channel_id & OPEN_CHANNEL_ID_MASK != 0;

        if unique_ids.contains(&self.current_user_id) {
            members.insert(
                self.current_user_id,
                self.get_current_user(&connection, channel_id, is_open_channel)?,
            );
        }

        let other_ids: Vec<i64> = unique_ids
            .into_iter()
            .filter(|id| *id != self.current_user_id)
            .collect();
        if is_open_channel {
            self.load_open_channel_users(&connection, channel_id, &other_ids, &mut members)?;
        } else {
            self.load_regular_users(&connection, &other_ids, &mut members)?;
        }

        user_ids
            .iter()
            .map(|user_id| {
                members
                    .get(user_id)
                    .cloned()
                    .ok_or_else(|| format!("member not found: {user_id}"))
            })
            .collect()
    }

    fn get_current_user(
        &self,
        connection: &rusqlite::Connection,
        channel_id: i64,
        is_open_channel: bool,
    ) -> Result<Member, String> {
        if !is_open_channel {
            return Ok(Member {
                id: self.current_user_id,
                is_mine: true,
                nickname: "Arisa".to_string(),
                profile_image_url: None,
                open_channel: None,
            });
        }

        connection
            .query_row(
                "SELECT user_id, profile_type, link_member_type, nickname,
                        o_profile_image_url, profile_link_id
                 FROM db2.open_profile
                 WHERE link_id = (SELECT link_id FROM db1.chat_rooms WHERE id = ?1)
                LIMIT 1",
                [channel_id],
                |row| {
                    Ok(Member {
                        id: row.get(0)?,
                        is_mine: true,
                        nickname: "Arisa".to_string(),
                        profile_image_url: row.get(4)?,
                        open_channel: Some(OpenChannelMemberExtra {
                            profile_type: profile_type(row.get(1)?),
                            link_member_type: link_member_type(row.get(2)?),
                            profile_link_id: row.get(5)?,
                        }),
                    })
                },
            )
            .map_err(|error| format!("open profile query failed: {error}"))
    }

    fn load_regular_users(
        &self,
        connection: &rusqlite::Connection,
        user_ids: &[i64],
        members: &mut HashMap<i64, Member>,
    ) -> Result<(), String> {
        for chunk in user_ids.chunks(500) {
            let sql = format!(
                "SELECT id, nickname, original_profile_image_url
                 FROM user.user WHERE id IN ({})",
                placeholders(chunk.len())
            );
            let mut statement = connection
                .prepare(&sql)
                .map_err(|error| format!("member query prepare failed: {error}"))?;
            let rows = statement
                .query_map(rusqlite::params_from_iter(chunk), |row| {
                    Ok(Member {
                        id: row.get(0)?,
                        nickname: row.get(1)?,
                        is_mine: false,
                        profile_image_url: row.get(2)?,
                        open_channel: None,
                    })
                })
                .map_err(|error| format!("member query failed: {error}"))?;
            for row in rows {
                let member = row.map_err(|error| format!("member mapping failed: {error}"))?;
                members.insert(member.id, member);
            }
        }

        let missing: Vec<i64> = user_ids
            .iter()
            .copied()
            .filter(|id| !members.contains_key(id))
            .collect();
        for chunk in missing.chunks(500) {
            let sql = format!(
                "SELECT id, name, original_profile_image_url, enc
                 FROM db2.friends WHERE id IN ({})",
                placeholders(chunk.len())
            );
            let mut statement = connection
                .prepare(&sql)
                .map_err(|error| format!("friend query prepare failed: {error}"))?;
            let rows = statement
                .query_map(rusqlite::params_from_iter(chunk), |row| {
                    Ok((
                        Member {
                            id: row.get(0)?,
                            nickname: row.get(1)?,
                            is_mine: false,
                            profile_image_url: row.get(2)?,
                            open_channel: None,
                        },
                        row.get(3)?,
                    ))
                })
                .map_err(|error| format!("friend query failed: {error}"))?;
            for row in rows {
                let (mut member, enc) =
                    row.map_err(|error| format!("friend mapping failed: {error}"))?;
                member.nickname = self.decryptor.decrypt(&member.nickname, enc);
                member.profile_image_url = member
                    .profile_image_url
                    .map(|url| self.decryptor.decrypt(&url, enc));
                members.insert(member.id, member);
            }
        }
        Ok(())
    }

    fn load_open_channel_users(
        &self,
        connection: &rusqlite::Connection,
        channel_id: i64,
        user_ids: &[i64],
        members: &mut HashMap<i64, Member>,
    ) -> Result<(), String> {
        for chunk in user_ids.chunks(500) {
            let sql = format!(
                "SELECT user_id, profile_type, link_member_type, nickname,
                        original_profile_image_url, profile_link_id, enc
                 FROM db2.open_chat_member
                 WHERE link_id = (SELECT link_id FROM db1.chat_rooms WHERE id = ?)
                   AND user_id IN ({})",
                placeholders(chunk.len())
            );
            let mut params = Vec::with_capacity(chunk.len() + 1);
            params.push(channel_id);
            params.extend_from_slice(chunk);
            let mut statement = connection
                .prepare(&sql)
                .map_err(|error| format!("open channel member prepare failed: {error}"))?;
            let rows = statement
                .query_map(rusqlite::params_from_iter(params), |row| {
                    Ok((
                        Member {
                            id: row.get(0)?,
                            is_mine: false,
                            nickname: row.get(3)?,
                            profile_image_url: row.get(4)?,
                            open_channel: Some(OpenChannelMemberExtra {
                                profile_type: profile_type(row.get(1)?),
                                link_member_type: link_member_type(row.get(2)?),
                                profile_link_id: row.get(5)?,
                            }),
                        },
                        row.get(6)?,
                    ))
                })
                .map_err(|error| format!("open channel member query failed: {error}"))?;
            for row in rows {
                let (mut member, enc) =
                    row.map_err(|error| format!("open channel member mapping failed: {error}"))?;
                member.nickname = self.decryptor.decrypt(&member.nickname, enc);
                member.profile_image_url = member
                    .profile_image_url
                    .map(|url| self.decryptor.decrypt(&url, enc));
                members.insert(member.id, member);
            }
        }
        Ok(())
    }

    pub fn get_channel(&self, channel_id: i64) -> Result<Channel, String> {
        let connection = self.connection()?;
        let (channel_type, private_meta, metadata): (String, Option<String>, Option<String>) =
            connection
                .query_row(
                    "SELECT type, private_meta, v FROM db1.chat_rooms WHERE id = ?1 LIMIT 1",
                    [channel_id],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .map_err(|error| format!("channel query failed: {error}"))?;

        let name = if channel_type == "OM" {
            Some(
                connection
                    .query_row(
                        "SELECT name FROM db2.open_link
                         WHERE id = (SELECT link_id FROM db1.chat_rooms WHERE id = ?1)
                         LIMIT 1",
                        [channel_id],
                        |row| row.get(0),
                    )
                    .map_err(|error| format!("open channel name query failed: {error}"))?,
            )
        } else {
            metadata
                .as_deref()
                .and_then(parse_display_user_ids)
                .map(|user_ids| {
                    self.get_users(channel_id, &user_ids).map(|members| {
                        members
                            .iter()
                            .map(|member| member.nickname.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    })
                })
                .transpose()?
        };

        Ok(Channel {
            id: channel_id,
            channel_type,
            name,
            private_name: private_meta.as_deref().and_then(parse_private_name),
        })
    }

    pub fn get_channel_members(&self, channel_id: i64) -> Result<ChannelMembers, String> {
        let connection = self.connection()?;
        let active_member_ids = connection
            .query_row(
                "SELECT active_member_ids FROM db1.chat_rooms WHERE id = ?1 LIMIT 1",
                [channel_id],
                |row| row.get::<_, Option<String>>(0),
            )
            .map_err(|error| format!("channel member query failed: {error}"))?
            .as_deref()
            .and_then(parse_i64_array)
            .unwrap_or_default();
        Ok(ChannelMembers {
            channel_id,
            active_member_ids,
        })
    }

    pub fn latest_message_database_id(&self) -> i64 {
        self.connection()
            .ok()
            .and_then(|connection| {
                connection
                    .query_row(
                        "SELECT COALESCE(MAX(_id), 0) FROM db1.chat_logs",
                        [],
                        |row| row.get(0),
                    )
                    .ok()
            })
            .unwrap_or_default()
    }

    pub fn messages_after(&self, database_id: i64, limit: i64) -> Result<Vec<MessageRow>, String> {
        let connection = self.connection()?;
        let mut statement = connection
            .prepare(
                "SELECT _id, id, type, chat_id, user_id, message, attachment, v, thread_id, scope
                 FROM db1.chat_logs WHERE _id > ?1 ORDER BY _id ASC LIMIT ?2",
            )
            .map_err(|error| format!("message query prepare failed: {error}"))?;
        collect_messages(
            statement
                .query_map([database_id, limit], read_message)
                .map_err(|error| format!("message query failed: {error}"))?,
        )
    }

    pub fn get_message(&self, channel_id: i64, message_id: i64) -> Result<Event, String> {
        self.get_messages(channel_id, &[message_id])?
            .into_iter()
            .next()
            .ok_or_else(|| format!("message not found: {message_id}"))
    }

    pub fn get_messages(&self, channel_id: i64, message_ids: &[i64]) -> Result<Vec<Event>, String> {
        self.get_message_rows(channel_id, message_ids)?
            .into_iter()
            .map(|row| self.map_message(row))
            .collect()
    }

    fn get_message_rows(
        &self,
        channel_id: i64,
        message_ids: &[i64],
    ) -> Result<Vec<MessageRow>, String> {
        if message_ids.is_empty() {
            return Ok(Vec::new());
        }

        let connection = self.connection()?;
        let unique_ids: HashSet<i64> = message_ids.iter().copied().collect();
        let mut messages = HashMap::new();
        for chunk in unique_ids.into_iter().collect::<Vec<_>>().chunks(500) {
            let sql = format!(
                "SELECT _id, id, type, chat_id, user_id, message, attachment, v, thread_id, scope
                 FROM db1.chat_logs WHERE chat_id = ? AND id IN ({})",
                placeholders(chunk.len())
            );
            let mut params = Vec::with_capacity(chunk.len() + 1);
            params.push(channel_id);
            params.extend_from_slice(chunk);
            let mut statement = connection
                .prepare(&sql)
                .map_err(|error| format!("message query prepare failed: {error}"))?;
            let rows = statement
                .query_map(rusqlite::params_from_iter(params), read_message)
                .map_err(|error| format!("message query failed: {error}"))?;
            for row in rows {
                let message =
                    row.map_err(|error| format!("message row mapping failed: {error}"))?;
                messages.insert(message.message_id, message);
            }
        }

        message_ids
            .iter()
            .map(|message_id| {
                messages
                    .get(message_id)
                    .cloned()
                    .ok_or_else(|| format!("message not found: {message_id}"))
            })
            .collect()
    }

    pub fn raw_query(&self, sql: &str, limit: usize) -> Result<Vec<Value>, String> {
        let connection = self.connection()?;
        let mut statement = connection
            .prepare(sql)
            .map_err(|error| format!("raw query prepare failed: {error}"))?;
        if !statement.readonly() {
            return Err("only read-only queries are allowed".to_string());
        }

        let columns: Vec<String> = statement
            .column_names()
            .iter()
            .map(ToString::to_string)
            .collect();
        let mut rows = statement
            .query([])
            .map_err(|error| format!("raw query failed: {error}"))?;
        let mut output = Vec::new();
        while output.len() < limit {
            let Some(row) = rows
                .next()
                .map_err(|error| format!("raw query row failed: {error}"))?
            else {
                break;
            };
            let mut object = serde_json::Map::new();
            for (index, name) in columns.iter().enumerate() {
                let value = row
                    .get_ref(index)
                    .map_err(|error| format!("raw query column failed: {error}"))?;
                object.insert(name.clone(), sqlite_value_to_json(value));
            }
            output.push(Value::Object(object));
        }
        Ok(output)
    }

    fn connection(
        &self,
    ) -> Result<r2d2::PooledConnection<r2d2_sqlite::SqliteConnectionManager>, String> {
        self.pool
            .get()
            .map_err(|error| format!("failed to get database connection: {error}"))
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

fn collect_messages(
    rows: rusqlite::MappedRows<'_, impl FnMut(&Row<'_>) -> rusqlite::Result<MessageRow>>,
) -> Result<Vec<MessageRow>, String> {
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|error| format!("message row mapping failed: {error}"))
}

fn profile_type(value: i32) -> i32 {
    match value {
        1 => ProfileType::Default as i32,
        2 | 4 => ProfileType::Kakao as i32,
        16 => ProfileType::Open as i32,
        _ => ProfileType::Unknown as i32,
    }
}

fn link_member_type(value: i32) -> i32 {
    match value {
        1 => LinkMemberType::Host as i32,
        2 => LinkMemberType::Member as i32,
        4 => LinkMemberType::Moderator as i32,
        8 => LinkMemberType::Bot as i32,
        _ => LinkMemberType::Unknown as i32,
    }
}

fn placeholders(length: usize) -> String {
    std::iter::repeat_n("?", length)
        .collect::<Vec<_>>()
        .join(",")
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

fn sqlite_value_to_json(value: ValueRef<'_>) -> Value {
    match value {
        ValueRef::Null => Value::Null,
        ValueRef::Integer(value) => Value::String(value.to_string()),
        ValueRef::Real(value) => json!(value),
        ValueRef::Text(value) => Value::String(String::from_utf8_lossy(value).into_owned()),
        ValueRef::Blob(value) => json!({ "blob_base64": STANDARD.encode(value) }),
    }
}

use std::collections::{HashMap, HashSet};

use r2d2_sqlite::rusqlite::{self, Connection, Row};

use crate::proto::{LinkMemberType, Member, OpenChannelMemberExtra, ProfileType};

use super::{Database, DbError, DbResult, OPEN_CHANNEL_ID_MASK, placeholders};

impl Database {
    pub fn get_user(&self, channel_id: i64, user_id: i64) -> DbResult<Member> {
        self.get_users(channel_id, &[user_id])?
            .into_iter()
            .next()
            .ok_or(DbError::MemberNotFound(user_id))
    }

    pub fn get_users(&self, channel_id: i64, user_ids: &[i64]) -> DbResult<Vec<Member>> {
        if user_ids.is_empty() {
            return Ok(Vec::new());
        }

        let connection = self.connection()?;
        self.get_users_with_connection(&connection, channel_id, user_ids)
    }

    pub(super) fn get_users_with_connection(
        &self,
        connection: &Connection,
        channel_id: i64,
        user_ids: &[i64],
    ) -> DbResult<Vec<Member>> {
        let mut unique_ids: HashSet<i64> = user_ids.iter().copied().collect();
        let mut members = HashMap::with_capacity(unique_ids.len());
        let is_open_channel = channel_id & OPEN_CHANNEL_ID_MASK != 0;

        if unique_ids.remove(&self.current_user_id) {
            let member = self.get_current_user(connection, channel_id, is_open_channel)?;
            members.insert(self.current_user_id, member);
        }

        let other_ids: Vec<i64> = unique_ids.into_iter().collect();

        if is_open_channel {
            self.load_open_channel_users(connection, channel_id, &other_ids, &mut members)?;
        } else {
            self.load_regular_users(connection, &other_ids, &mut members)?;
        }

        user_ids
            .iter()
            .map(|user_id| {
                members
                    .get(user_id)
                    .cloned()
                    .ok_or(DbError::MemberNotFound(*user_id))
            })
            .collect()
    }

    fn get_current_user(
        &self,
        connection: &Connection,
        channel_id: i64,
        is_open_channel: bool,
    ) -> DbResult<Member> {
        if !is_open_channel {
            return Ok(Member {
                id: self.current_user_id,
                is_mine: true,
                nickname: "Arisa".to_owned(),
                profile_image_url: None,
                open_channel: None,
            });
        }

        Ok(connection.query_row(
            "SELECT user_id, profile_type, link_member_type,
                    o_profile_image_url, profile_link_id
             FROM db2.open_profile
             WHERE link_id = (SELECT link_id FROM db1.chat_rooms WHERE id = ?1)
             LIMIT 1",
            [channel_id],
            read_current_open_profile,
        )?)
    }

    fn load_regular_users(
        &self,
        connection: &Connection,
        user_ids: &[i64],
        members: &mut HashMap<i64, Member>,
    ) -> DbResult<()> {
        for chunk in user_ids.chunks(500) {
            let sql = format!(
                "SELECT id, nickname, original_profile_image_url
                 FROM user.user WHERE id IN ({})",
                placeholders(chunk.len()),
            );

            let mut statement = connection.prepare(&sql)?;
            let rows = statement.query_map(rusqlite::params_from_iter(chunk), read_regular_user)?;

            for row in rows {
                let member = row?;
                members.insert(member.id, member);
            }
        }

        let missing_ids: Vec<i64> = user_ids
            .iter()
            .copied()
            .filter(|id| !members.contains_key(id))
            .collect();

        for chunk in missing_ids.chunks(500) {
            let sql = format!(
                "SELECT id, name, original_profile_image_url, enc
                 FROM db2.friends WHERE id IN ({})",
                placeholders(chunk.len()),
            );

            let mut statement = connection.prepare(&sql)?;
            let rows = statement.query_map(rusqlite::params_from_iter(chunk), |row| {
                self.read_friend(row)
            })?;

            for row in rows {
                let member = row?;
                members.insert(member.id, member);
            }
        }

        Ok(())
    }

    fn load_open_channel_users(
        &self,
        connection: &Connection,
        channel_id: i64,
        user_ids: &[i64],
        members: &mut HashMap<i64, Member>,
    ) -> DbResult<()> {
        for chunk in user_ids.chunks(500) {
            let sql = format!(
                "SELECT user_id, profile_type, link_member_type, nickname,
                        original_profile_image_url, profile_link_id, enc
                 FROM db2.open_chat_member
                 WHERE link_id = (SELECT link_id FROM db1.chat_rooms WHERE id = ?)
                   AND user_id IN ({})",
                placeholders(chunk.len()),
            );

            let mut params = Vec::with_capacity(chunk.len() + 1);
            params.push(channel_id);
            params.extend_from_slice(chunk);

            let mut statement = connection.prepare(&sql)?;
            let rows = statement.query_map(rusqlite::params_from_iter(params), |row| {
                self.read_open_channel_user(row)
            })?;

            for row in rows {
                let member = row?;
                members.insert(member.id, member);
            }
        }

        Ok(())
    }

    fn read_friend(&self, row: &Row<'_>) -> rusqlite::Result<Member> {
        let nickname: String = row.get(1)?;
        let profile_image_url: Option<String> = row.get(2)?;
        let enc = row.get(3)?;

        Ok(Member {
            id: row.get(0)?,
            is_mine: false,
            nickname: self.decryptor.decrypt(&nickname, enc),
            profile_image_url: profile_image_url.map(|url| self.decryptor.decrypt(&url, enc)),
            open_channel: None,
        })
    }

    fn read_open_channel_user(&self, row: &Row<'_>) -> rusqlite::Result<Member> {
        let nickname: String = row.get(3)?;
        let profile_image_url: Option<String> = row.get(4)?;
        let enc = row.get(6)?;

        Ok(Member {
            id: row.get(0)?,
            is_mine: false,
            nickname: self.decryptor.decrypt(&nickname, enc),
            profile_image_url: profile_image_url.map(|url| self.decryptor.decrypt(&url, enc)),
            open_channel: Some(OpenChannelMemberExtra {
                profile_type: profile_type(row.get(1)?),
                link_member_type: link_member_type(row.get(2)?),
                profile_link_id: row.get(5)?,
            }),
        })
    }
}

fn read_regular_user(row: &Row<'_>) -> rusqlite::Result<Member> {
    Ok(Member {
        id: row.get(0)?,
        is_mine: false,
        nickname: row.get(1)?,
        profile_image_url: row.get(2)?,
        open_channel: None,
    })
}

fn read_current_open_profile(row: &Row<'_>) -> rusqlite::Result<Member> {
    Ok(Member {
        id: row.get(0)?,
        is_mine: true,
        nickname: "Arisa".to_owned(),
        profile_image_url: row.get(3)?,
        open_channel: Some(OpenChannelMemberExtra {
            profile_type: profile_type(row.get(1)?),
            link_member_type: link_member_type(row.get(2)?),
            profile_link_id: row.get(4)?,
        }),
    })
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

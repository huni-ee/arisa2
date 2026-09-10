use serde_json::Value;

use crate::{
    crypto::crypto::Decryptor,
    database::{feed, record::MessageRow},
    proto::{
        Channel, Event, FeedEvent, FeedPayload, Member, MessageEvent, MessageRevision,
        MessageScope, event, feed_payload,
    },
};

use super::{Database, DbError, DbResult};

impl Database {
    pub fn decrypt(&self, ciphertext: &str, enc: u32, user_id: Option<i64>) -> String {
        match user_id {
            Some(user_id) => self.decryptor.decrypt_for_user(ciphertext, enc, user_id),
            None => self.decryptor.decrypt(ciphertext, enc),
        }
    }

    pub(crate) fn map_message(&self, row: MessageRow) -> DbResult<Event> {
        let author = self.get_user(row.channel_id, row.user_id).ok();
        let channel = self.get_channel(row.channel_id)?;

        if row.message_type != 0 {
            return assemble_message(&self.decryptor, &row, channel, author);
        }

        let mut event = assemble_message(&self.decryptor, &row, channel.clone(), author.clone())?;

        if let Some(event::Value::Feed(feed)) = event.value.as_mut()
            && let Some(payload) = feed.feed.as_mut()
        {
            self.enrich_feed(&row, payload, &channel, author.as_ref())?;
        }

        Ok(event)
    }

    fn enrich_feed(
        &self,
        row: &MessageRow,
        payload: &mut FeedPayload,
        channel: &Channel,
        author: Option<&Member>,
    ) -> DbResult<()> {
        let Some(value) = payload.value.as_mut() else {
            return Ok(());
        };

        match value {
            feed_payload::Value::MessageDeleted(deleted) => {
                deleted.previous_message = if deleted.message_id == row.message_id {
                    None
                } else {
                    self.get_message_snapshot(row.channel_id, deleted.message_id)?
                };
            }
            feed_payload::Value::MessageChanged(changed) => {
                changed.message = if changed.message_id == row.message_id {
                    None
                } else {
                    self.get_message_snapshot(row.channel_id, changed.message_id)?
                };
            }
            feed_payload::Value::MessageHidden(hidden) => {
                let mut previous_messages = hidden
                    .message_ids
                    .iter()
                    .copied()
                    .filter(|message_id| *message_id != row.message_id)
                    .map(|message_id| self.get_message_snapshot(row.channel_id, message_id))
                    .collect::<DbResult<Vec<_>>>()?
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>();

                if let Some(metadata_message) = hidden_message_from_metadata(&self.decryptor, row) {
                    let target_message_id = hidden
                        .message_ids
                        .iter()
                        .copied()
                        .find(|message_id| *message_id == row.message_id)
                        .or_else(|| hidden.message_ids.first().copied());

                    if let Some(message_id) = target_message_id {
                        if let Some(previous_message) = previous_messages
                            .iter_mut()
                            .find(|message| message.message_id == message_id)
                        {
                            previous_message.message = metadata_message.message;
                        } else {
                            previous_messages.push(MessageEvent {
                                message_id,
                                thread_id: row.thread_id,
                                scope: map_scope(row.scope),
                                message_type: row.message_type,
                                channel: Some(channel.clone()),
                                author: author.cloned(),
                                message: metadata_message.message,
                                attachment_json: "{}".to_string(),
                                modify_log: Vec::new(),
                            });
                        }
                    }
                }

                hidden.previous_messages = previous_messages;
            }
            _ => {}
        }

        Ok(())
    }

    fn get_message_snapshot(
        &self,
        channel_id: i64,
        message_id: i64,
    ) -> DbResult<Option<MessageEvent>> {
        match self.get_message(channel_id, message_id) {
            Ok(Event {
                value: Some(event::Value::Message(message)),
            }) => Ok(Some(message)),
            Ok(Event {
                value: Some(event::Value::Feed(feed)),
            }) => Ok(feed.feed.and_then(|payload| match payload.value {
                Some(feed_payload::Value::MessageHidden(hidden)) => hidden
                    .previous_messages
                    .into_iter()
                    .find(|message| message.message_id == message_id),
                _ => None,
            })),
            Ok(_) | Err(DbError::MessageNotFound(_)) => Ok(None),
            Err(error) => Err(error),
        }
    }
}

struct HiddenMessageMetadata {
    message: String,
}

fn hidden_message_from_metadata(
    decryptor: &Decryptor,
    row: &MessageRow,
) -> Option<HiddenMessageMetadata> {
    if row.message_type != 0 {
        return None;
    }

    let metadata: Value = serde_json::from_str(row.metadata.as_deref()?).ok()?;
    let ciphertext = metadata.get("previous_message")?.as_str()?;
    let enc = metadata_u32(&metadata, "previous_enc")?;
    let message = decryptor.decrypt_for_user(ciphertext, enc, row.user_id);

    Some(HiddenMessageMetadata { message })
}

fn metadata_u32(metadata: &Value, field: &str) -> Option<u32> {
    let value = metadata.get(field)?;
    value
        .as_u64()
        .and_then(|value| value.try_into().ok())
        .or_else(|| value.as_str()?.parse().ok())
}

fn map_modify_log(
    decryptor: &Decryptor,
    metadata: Option<&Value>,
    user_id: i64,
) -> Vec<MessageRevision> {
    let Some(metadata) = metadata else {
        return Vec::new();
    };
    let Some(modify_log) = metadata.get("modifyLog") else {
        return Vec::new();
    };
    let entries: Value = match modify_log {
        Value::String(value) => {
            let Ok(entries) = serde_json::from_str(value) else {
                return Vec::new();
            };
            entries
        }
        Value::Array(_) => modify_log.clone(),
        _ => return Vec::new(),
    };

    let mut revisions: Vec<_> = entries
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            let revision = entry.get("revision")?.as_i64()?;
            let ciphertext = entry.get("message")?.as_str()?;
            let enc = metadata_u32(entry, "enc")?;
            Some(MessageRevision {
                revision,
                message: decryptor.decrypt_for_user(ciphertext, enc, user_id),
            })
        })
        .collect();
    revisions.sort_by_key(|entry| entry.revision);
    revisions
}

fn assemble_message(
    decryptor: &Decryptor,
    row: &MessageRow,
    channel: Channel,
    author: Option<Member>,
) -> DbResult<Event> {
    let metadata: Option<Value> = row
        .metadata
        .as_deref()
        .and_then(|value| serde_json::from_str(value).ok());

    let enc = metadata
        .as_ref()
        .and_then(|value| value.get("enc"))
        .and_then(Value::as_u64)
        .unwrap_or_default() as u32;

    let message = row
        .message
        .as_deref()
        .map(|value| decryptor.decrypt_for_user(value, enc, row.user_id))
        .unwrap_or_default();

    let attachment = row
        .attachment
        .as_deref()
        .map(|value| decryptor.decrypt_for_user(value, enc, row.user_id))
        .unwrap_or_default();

    let attachment_json = serde_json::from_str::<Value>(&attachment)
        .unwrap_or_else(|_| serde_json::json!({}))
        .to_string();

    let value = if row.message_type == 0 {
        let payload = feed::parse(&message, author.as_ref(), row.message_id)
            .map_err(DbError::MessageMapping)?;

        event::Value::Feed(FeedEvent {
            channel: Some(channel),
            feed: Some(payload),
            author,
        })
    } else {
        let modify_log = map_modify_log(decryptor, metadata.as_ref(), row.user_id);

        event::Value::Message(MessageEvent {
            message_id: row.message_id,
            thread_id: row.thread_id,
            scope: map_scope(row.scope),
            message_type: row.message_type,
            channel: Some(channel),
            author,
            message,
            attachment_json,
            modify_log,
        })
    };

    Ok(Event { value: Some(value) })
}

fn map_scope(scope: i32) -> i32 {
    match scope {
        1 => MessageScope::Channel as i32,
        2 => MessageScope::Thread as i32,
        3 => MessageScope::All as i32,
        _ => MessageScope::Unknown as i32,
    }
}

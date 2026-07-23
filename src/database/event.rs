use serde_json::Value;

use crate::proto::{Channel, Event, FeedEvent, Member, MessageEvent, MessageScope, event};

use super::{Database, Decryptor, feed, record::MessageRow};

impl Database {
    pub fn decrypt(&self, ciphertext: &str, enc: u32, user_id: Option<i64>) -> String {
        match user_id {
            Some(user_id) => self.decryptor.decrypt_for_user(ciphertext, enc, user_id),
            None => self.decryptor.decrypt(ciphertext, enc),
        }
    }

    pub(crate) fn map_message(&self, row: MessageRow) -> Result<Event, String> {
        let author = self.get_user(row.channel_id, row.user_id).ok();
        let channel = self.get_channel(row.channel_id)?;
        assemble_message(&self.decryptor, row, channel, author)
    }
}

fn assemble_message(
    decryptor: &Decryptor,
    row: MessageRow,
    channel: Channel,
    author: Option<Member>,
) -> Result<Event, String> {
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
        let payload = feed::parse(&message, author.as_ref())?;
        event::Value::Feed(FeedEvent {
            channel: Some(channel),
            feed: Some(payload),
            author,
        })
    } else {
        event::Value::Message(MessageEvent {
            message_id: row.message_id,
            thread_id: row.thread_id,
            scope: map_scope(row.scope),
            message_type: row.message_type,
            channel: Some(channel),
            author,
            message,
            attachment_json,
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

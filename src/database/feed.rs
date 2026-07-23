use serde_json::Value;

use crate::proto::{
    FeedHostChanged, FeedManagerSpeakerMode, FeedMessageChanged, FeedMessageDeleted,
    FeedMessageHidden, FeedModeratorAdded, FeedModeratorRemoved, FeedOpenLinkDeleted, FeedPayload,
    FeedUnknown, FeedUser, FeedUserJoined, FeedUserKicked, FeedUserLeft, Member, feed_payload,
};

#[derive(Clone, Copy)]
enum FeedType {
    Unknown,
    UserJoined,
    UserLeft,
    OpenLinkUserJoined,
    OpenLinkDeleted,
    UserKicked,
    ModeratorAdded,
    ModeratorRemoved,
    MessageDeleted,
    HostChanged,
    MessageChanged,
    MessageHidden,
    ManagerSpeakerMode,
}

impl From<i64> for FeedType {
    fn from(value: i64) -> Self {
        match value {
            1 => Self::UserJoined,
            2 => Self::UserLeft,
            4 => Self::OpenLinkUserJoined,
            5 => Self::OpenLinkDeleted,
            6 => Self::UserKicked,
            11 => Self::ModeratorAdded,
            12 => Self::ModeratorRemoved,
            14 => Self::MessageDeleted,
            15 => Self::HostChanged,
            25 => Self::MessageChanged,
            26 => Self::MessageHidden,
            27 => Self::ManagerSpeakerMode,
            _ => Self::Unknown,
        }
    }
}

pub fn parse(raw: &str, author: Option<&Member>) -> Result<FeedPayload, String> {
    let json: Value =
        serde_json::from_str(raw).map_err(|error| format!("invalid feed JSON: {error}"))?;
    let feed_type = json
        .get("feedType")
        .and_then(Value::as_i64)
        .map(FeedType::from)
        .unwrap_or(FeedType::Unknown);

    let value = match feed_type {
        FeedType::UserJoined | FeedType::OpenLinkUserJoined => {
            feed_payload::Value::UserJoined(FeedUserJoined {
                joined_users: required_array(&json, "members")?
                    .iter()
                    .filter_map(feed_user)
                    .collect(),
            })
        }
        FeedType::UserLeft => feed_payload::Value::UserLeft(FeedUserLeft {
            left_member: json.get("member").and_then(feed_user),
        }),
        FeedType::UserKicked => feed_payload::Value::UserKicked(FeedUserKicked {
            kicked_user: json.get("member").and_then(feed_user),
            kicked_by: author.cloned(),
        }),
        FeedType::MessageDeleted => feed_payload::Value::MessageDeleted(FeedMessageDeleted {
            message_id: required_id(&json, "logId")?,
        }),
        FeedType::MessageChanged => feed_payload::Value::MessageChanged(FeedMessageChanged {
            message_id: required_id(&json, "logId")?,
            target_revision: required_i64(&json, "targetRevision")?,
        }),
        FeedType::MessageHidden => feed_payload::Value::MessageHidden(FeedMessageHidden {
            message_ids: required_array(&json, "chatLogInfos")?
                .iter()
                .filter_map(|value| value.get("logId").and_then(json_id))
                .collect(),
        }),
        FeedType::HostChanged => feed_payload::Value::HostChanged(FeedHostChanged {
            previous_host: json.get("prevHost").and_then(feed_user),
            new_host: json.get("newHost").and_then(feed_user),
        }),
        FeedType::ModeratorAdded => feed_payload::Value::ModeratorAdded(FeedModeratorAdded {
            member: json.get("member").and_then(feed_user),
        }),
        FeedType::ModeratorRemoved => feed_payload::Value::ModeratorRemoved(FeedModeratorRemoved {
            member: json.get("member").and_then(feed_user),
        }),
        FeedType::ManagerSpeakerMode => {
            let enabled = match required_i64(&json, "eventType")? {
                1 => true,
                2 => false,
                value => return Err(format!("invalid manager speaker mode: {value}")),
            };
            feed_payload::Value::ManagerSpeakerMode(FeedManagerSpeakerMode { enabled })
        }
        FeedType::OpenLinkDeleted => feed_payload::Value::OpenLinkDeleted(FeedOpenLinkDeleted {}),
        FeedType::Unknown => feed_payload::Value::Unknown(FeedUnknown {
            raw_json: json.to_string(),
        }),
    };

    Ok(FeedPayload { value: Some(value) })
}

fn required_array<'a>(value: &'a Value, field: &str) -> Result<&'a Vec<Value>, String> {
    value
        .get(field)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("feed is missing array field {field}"))
}

fn required_i64(value: &Value, field: &str) -> Result<i64, String> {
    value
        .get(field)
        .and_then(Value::as_i64)
        .ok_or_else(|| format!("feed is missing integer field {field}"))
}

fn required_id(value: &Value, field: &str) -> Result<i64, String> {
    value
        .get(field)
        .and_then(json_id)
        .ok_or_else(|| format!("feed is missing integer field {field}"))
}

fn json_id(value: &Value) -> Option<i64> {
    value.as_i64().or_else(|| value.as_str()?.parse().ok())
}

fn feed_user(value: &Value) -> Option<FeedUser> {
    Some(FeedUser {
        id: json_id(value.get("userId")?)?,
        nickname: value.get("nickName")?.as_str()?.to_string(),
    })
}

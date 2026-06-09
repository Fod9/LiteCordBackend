use serde::{Deserialize, Serialize};
use surrealdb::sql::{Datetime, Thing};

mod ser {
    use serde::Serializer;
    use surrealdb::sql::Thing;

    pub fn thing<S: Serializer>(val: &Thing, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&val.to_raw())
    }

    pub fn opt_thing<S: Serializer>(val: &Option<Thing>, s: S) -> Result<S::Ok, S::Error> {
        match val {
            Some(t) => s.serialize_some(&t.to_raw()),
            None => s.serialize_none(),
        }
    }

    pub fn vec_thing<S: Serializer>(val: &Vec<Thing>, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeSeq;
        let mut seq = s.serialize_seq(Some(val.len()))?;
        for t in val {
            seq.serialize_element(&t.to_raw())?;
        }
        seq.end()
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub enum ActivityStatus {
    Online,
    Offline,
    Invisible,
    DoNotDisturb,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum ChannelType {
    Text,
    Voice,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Role {
    #[serde(serialize_with = "ser::opt_thing")]
    pub id: Option<Thing>,
    #[serde(serialize_with = "ser::thing")]
    pub guild: Thing,
    pub name: String,
    pub color: String,
    pub position: i32,
    pub permissions: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct User {
    #[serde(serialize_with = "ser::opt_thing")]
    pub id: Option<Thing>,
    pub name: String,
    pub display_name: String,
    pub profile_picture: String,
    pub email: String,
    pub password: String,
    pub status: ActivityStatus,
    pub created_at: Datetime,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SimpleUser {
    #[serde(serialize_with = "ser::thing")]
    pub id: Thing,
    pub name: String,
    pub display_name: String,
    pub profile_picture: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct RefreshToken {
    #[serde(serialize_with = "ser::opt_thing")]
    pub id: Option<Thing>,
    #[serde(serialize_with = "ser::thing")]
    pub user: Thing,
    pub token: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct DMChannel {
    #[serde(serialize_with = "ser::opt_thing")]
    pub id: Option<Thing>,
    #[serde(serialize_with = "ser::vec_thing")]
    pub recipients: Vec<Thing>,
    pub recipients_key: String,
    pub name: Option<String>,
    #[serde(serialize_with = "ser::thing")]
    pub owner: Thing,
    #[serde(serialize_with = "ser::opt_thing")]
    pub last_message_id: Option<Thing>,
    pub created_at: Datetime,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct DMChannelWithParticipants {
    #[serde(serialize_with = "ser::opt_thing")]
    pub id: Option<Thing>,
    #[serde(rename(deserialize = "recipients", serialize = "participants"))]
    pub participants: Vec<SimpleUser>,
    pub recipients_key: String,
    pub name: Option<String>,
    #[serde(serialize_with = "ser::thing")]
    pub owner: Thing,
    #[serde(serialize_with = "ser::opt_thing")]
    pub last_message_id: Option<Thing>,
    pub created_at: Datetime,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct FriendshipWithUsers {
    pub id: Option<Thing>,
    pub in_user: SimpleUser,
    pub out_user: SimpleUser,
    pub status: String,
    pub created_at: Datetime,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Friendship {
    #[serde(serialize_with = "ser::opt_thing")]
    pub id: Option<Thing>,

    #[serde(rename = "in", serialize_with = "ser::thing")]
    pub user_a: Thing,

    #[serde(rename = "out", serialize_with = "ser::thing")]
    pub user_b: Thing,

    pub created_at: Datetime,
    pub status: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct MemberOf {
    #[serde(serialize_with = "ser::opt_thing")]
    pub id: Option<Thing>,

    #[serde(rename = "in", serialize_with = "ser::thing")]
    pub user: Thing,

    #[serde(rename = "out", serialize_with = "ser::thing")]
    pub guild: Thing,

    #[serde(serialize_with = "ser::vec_thing")]
    pub roles: Vec<Thing>,
    pub nickname: Option<String>,
    pub joined_at: Datetime,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Emoji {
    #[serde(serialize_with = "ser::opt_thing")]
    pub id: Option<Thing>,
    #[serde(serialize_with = "ser::thing")]
    pub owner: Thing,
    #[serde(serialize_with = "ser::thing")]
    pub guild: Thing,
    pub name: String,
    pub image: String,
    pub created_at: Datetime,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Guild {
    #[serde(serialize_with = "ser::opt_thing")]
    pub id: Option<Thing>,
    pub name: String,
    pub icon: String,
    #[serde(serialize_with = "ser::thing")]
    pub owner: Thing,
    pub created_at: Datetime,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct GuildInvite {
    #[serde(serialize_with = "ser::opt_thing")]
    pub id: Option<Thing>,
    #[serde(serialize_with = "ser::thing")]
    pub guild: Thing,
    #[serde(serialize_with = "ser::thing")]
    pub inviter: Thing,
    pub code: String,
    pub expires_at: Option<Datetime>,
    pub created_at: Datetime,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Channel {
    #[serde(serialize_with = "ser::opt_thing")]
    pub id: Option<Thing>,
    #[serde(serialize_with = "ser::thing")]
    pub guild: Thing,
    pub name: String,
    pub channel_type: ChannelType,
    pub category: Option<String>,
    pub created_at: Datetime,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Attachment {
    pub url: String,
    pub filename: String,
    pub size: u64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct MemberProfile {
    #[serde(serialize_with = "ser::opt_thing")]
    pub id: Option<Thing>,
    pub user: SimpleUser,
    #[serde(serialize_with = "ser::vec_thing")]
    pub roles: Vec<Thing>,
    pub nickname: Option<String>,
    pub joined_at: Datetime,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct MessageWithAuthor {
    #[serde(serialize_with = "ser::opt_thing")]
    pub id: Option<Thing>,
    #[serde(serialize_with = "ser::thing")]
    pub channel: Thing,
    pub author: SimpleUser,
    pub content: String,
    #[serde(serialize_with = "ser::opt_thing")]
    pub reply_to: Option<Thing>,
    pub attachments: Vec<Attachment>,
    pub edited_at: Option<String>,
    pub created_at: Datetime,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Message {
    #[serde(serialize_with = "ser::opt_thing")]
    pub id: Option<Thing>,
    #[serde(serialize_with = "ser::thing")]
    pub channel: Thing,
    #[serde(serialize_with = "ser::thing")]
    pub author: Thing,
    pub content: String,
    #[serde(serialize_with = "ser::opt_thing")]
    pub reply_to: Option<Thing>,
    pub attachments: Vec<Attachment>,
    pub edited_at: Option<String>,
    pub created_at: Datetime,
}

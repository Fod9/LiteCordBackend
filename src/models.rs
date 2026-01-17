use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;

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
    pub id: Option<Thing>,
    pub guild: Thing,
    pub name: String,
    pub color: String,
    pub position: i32,
    pub permissions: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct User {
    pub id: Option<Thing>,
    pub name: String,
    pub display_name: String,
    pub profile_picture: String,
    pub email: String,
    pub password: String,
    pub status: ActivityStatus,
    pub created_at: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct DMChannel {
    pub id: Option<Thing>,
    pub recipients: Vec<Thing>,
    pub name: Option<String>,
    pub owner: Thing,
    pub last_message_id: Option<Thing>,
    pub created_at: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Friendship {
    pub id: Option<Thing>,

    #[serde(rename = "in")]
    pub user_a: Thing,

    #[serde(rename = "out")]
    pub user_b: Thing,

    pub created_at: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct MemberOf {
    pub id: Option<Thing>,

    #[serde(rename = "in")]
    pub user: Thing,

    #[serde(rename = "out")]
    pub guild: Thing,

    pub roles: Vec<Thing>,
    pub nickname: Option<String>,
    pub joined_at: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Emoji {
    pub id: Option<Thing>,
    pub owner: Thing,
    pub guild: Thing,
    pub name: String,
    pub image: String,
    pub created_at: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Guild {
    pub id: Option<Thing>,
    pub name: String,
    pub icon: String,
    pub owner: Thing,
    pub created_at: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct GuildInvite {
    pub id: Option<Thing>,
    pub guild: Thing,
    pub inviter: Thing,
    pub code: String,
    pub expires_at: Option<String>,
    pub created_at: String,
}
#[derive(Serialize, Deserialize, Debug)]
pub struct Channel {
    pub id: Option<Thing>,
    pub guild: Thing,
    pub name: String,
    pub channel_type: ChannelType,
    pub category: Option<String>,
    pub created_at: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Attachment {
    pub url: String,
    pub filename: String,
    pub size: u64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Message {
    pub id: Option<Thing>,
    pub channel: Thing,
    pub author: Thing,
    pub content: String,
    pub reply_to: Option<Thing>,
    pub attachments: Vec<Attachment>,
    pub edited_at: Option<String>,
    pub created_at: String,
}

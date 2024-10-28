use crate::ircsky::{ChannelName, ChannelUri, User};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Profile {
    #[serde(rename = "$type")]
    r#type: String,
    pub nickname: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Message {
    #[serde(rename = "$type")]
    pub r#type: String,
    pub content: String,
    //pub facets: Option<serde_json::Value>,
    pub room: ChannelUri,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Room {
    pub name: String,
    pub languages: Option<Vec<String>>,
    pub topic: Option<String>,
    pub tags: Option<Vec<String>>,
    pub allowlist: Option<ModList>,
    pub denylist: Option<ModList>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ModList {
    active: bool,
    users: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum PskyEvent {
    PrivateMessage(User, Message, ChannelName),
    //DeleteMessage(User),
    //ProfileUpdate(User, User),
    //HandleUpdate(User, User),
    Join(User, ChannelName),
    Part(User, ChannelName),
}

use anyhow::Result;
use dashmap::DashMap;
use std::collections::HashSet;
use std::ops::Deref;
use std::sync::Arc;

use crate::atproto;
use crate::config::Settings;
use crate::psky;

#[derive(Clone)]
pub struct Ircsky {
    pub users: Arc<DashMap<String, User>>,
    pub channels: Arc<DashMap<ChannelUri, Channel>>,
    channel_name_map: Arc<DashMap<ChannelName, ChannelUri>>,
    pub config: Arc<Settings>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ChannelName(pub String);
impl std::fmt::Display for ChannelName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
#[derive(Clone, PartialEq, Eq, Hash, Debug, serde::Deserialize, serde::Serialize)]
pub struct ChannelUri(pub String);

pub struct Channel {
    pub uri: ChannelUri,
    pub name: ChannelName,
    pub sender: tokio::sync::broadcast::Sender<psky::PskyEvent>,
    pub users: HashSet<String>,
    pub room: psky::Room,
}

impl Ircsky {
    pub fn new(config: Settings) -> Self {
        Self {
            users: Arc::new(DashMap::new()),
            channels: Arc::new(DashMap::new()),
            channel_name_map: Arc::new(DashMap::new()),
            config: Arc::new(config),
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        let _ = tokio::join!(
            self.clone().start_jetstream(),
            self.clone().start_irc_server()
        );

        Ok(())
    }

    pub async fn resolve_channel(&self, channel: &ChannelName) -> Option<ChannelUri> {
        if let Some(channel_uri) = self.channel_name_map.get(channel) {
            return Some(channel_uri.value().clone());
        }

        // parse channel name:
        // starts with #, then utf-8 chars, then @, then a handle
        let (head, tail) = channel.0.split_at(1);
        if head != "#" {
            return None;
        }

        let (_, handle) = tail.split_at(tail.find('@')?);
        let handle = &handle[1..]; // skip the @

        let did = atproto::resolve_handle(handle).await.ok()?;
        let pds = atproto::get_pds(&did).await.ok()?;
        // we get the handle's pds, call listRecords, insert every room they have

        #[derive(serde::Deserialize, Debug)]
        struct GetRoom {
            uri: String,
            value: psky::Room,
        }

        #[derive(serde::Deserialize, Debug)]
        struct ListRooms {
            records: Vec<GetRoom>,
        }

        // 3. from pds, get profile, cache
        let rooms = reqwest::get(&format!(
            "{}/xrpc/com.atproto.repo.listRecords?repo={}&collection=social.psky.chat.room",
            pds, did
        ))
        .await
        .ok()?
        .json::<ListRooms>()
        .await
        .map(|gp| gp.records)
        .ok()?;

        dbg!(&rooms);

        for room in rooms {
            self.channel_name_map.insert(
                ChannelName(format!("#{}@{}", room.value.name, handle)),
                ChannelUri(room.uri.clone()),
            );
            self.channels.insert(
                ChannelUri(room.uri.clone()),
                Channel {
                    uri: ChannelUri(room.uri),
                    name: ChannelName(format!("#{}@{}", room.value.name, handle)),
                    sender: tokio::sync::broadcast::channel(16).0,
                    users: HashSet::new(),
                    room: room.value,
                },
            );
        }

        self.channel_name_map
            .get(channel)
            .map(|uri| uri.value().clone())
    }

    pub async fn channel_name(&self, channel: &ChannelUri) -> Option<ChannelName> {
        Some(self.channels.get(channel)?.name.clone())
    }

    pub async fn get_user<'a>(&'a self, did: &str) -> Result<(impl AsRef<User> + 'a, bool)> {
        // 0. check cache
        if let Some(user) = self.users.get(did) {
            return Ok((user, true));
        }

        // 1. resolve did (get did doc)
        let did_doc = atproto::get_did_doc(did).await?;

        // 2. from did doc, get pds
        let pds = &did_doc
            .service
            .iter()
            .find(|s| s.id == "#atproto_pds" && s.r#type == "AtprotoPersonalDataServer")
            .ok_or(anyhow::anyhow!("pds not found"))?
            .service_endpoint;

        let mut claimed_handle = did_doc
            .also_known_as
            .iter()
            .find(|aka| aka.starts_with("at://"))
            .map(|aka| aka[5..].to_string());

        // verify
        if let Some(claimed) = &claimed_handle {
            let resolved_did = atproto::resolve_handle(claimed).await?;
            if resolved_did != did {
                claimed_handle = None;
            }
        }

        #[derive(serde::Deserialize)]
        struct GetProfile {
            value: psky::Profile,
        }

        // 3. from pds, get profile, cache
        let profile = reqwest::get(&format!(
            "{}/xrpc/com.atproto.repo.getRecord?repo={}&collection=social.psky.actor.profile&rkey=self",
            pds, did
        ))
        .await?
        .json::<GetProfile>()
        .await
        .map(|gp| gp.value)
        .ok();

        let ret = User {
            did: did.to_string(),
            profile,
            handle: claimed_handle,
            sender: None,
        };
        self.users.insert(did.to_string(), ret);
        Ok((
            self.users
                .get(did)
                .ok_or(anyhow::anyhow!("Inserted user gone"))?,
            false,
        ))
    }
}

impl AsRef<User> for dashmap::mapref::one::Ref<'_, String, User> {
    fn as_ref(&self) -> &User {
        self.deref()
    }
}

#[derive(Debug, Clone)]
pub struct User {
    pub did: String,
    pub profile: Option<psky::Profile>,
    pub handle: Option<String>,
    pub sender: Option<tokio::sync::broadcast::Sender<psky::PskyEvent>>,
}

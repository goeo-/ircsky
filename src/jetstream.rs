use anyhow::Result;
use fastwebsockets::{Frame, OpCode};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use crate::ircsky;
use crate::psky;
use crate::websocket::{self, FrameStream};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Account {
    active: bool,
    did: String,
    seq: u64,
    status: Option<String>,
    time: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Identity {
    did: String,
    pub handle: Option<String>,
    seq: u64,
    time: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Commit {
    rev: Option<String>,
    operation: String,
    pub collection: Option<String>,
    rkey: Option<String>,
    pub record: Option<serde_json::Value>,
    cid: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Event {
    pub did: String,
    pub time_us: u64,
    pub kind: String,
    pub commit: Option<Commit>,
    account: Option<Account>,
    pub identity: Option<Identity>,
}

impl ircsky::Ircsky {
    pub async fn start_jetstream(self) -> Result<()> {
        let config = &self.config.jetstream;
        let mut last_time: Option<u64> = None;
        loop {
            let mut path = String::from(
                "subscribe?wantedCollections=social.psky.chat.message&\
wantedCollections=social.psky.actor.profile&wantedCollections=social.psky.chat.room",
            );

            if let Some(cursor) = last_time {
                path.push_str(&format!("&cursor={}", cursor));
            }

            println!("connecting to {}:{}/{}", &config.host, config.port, &path);
            let mut ws = websocket::connect(&config.host, config.port, &path).await?;

            loop {
                let msg = match tokio::time::timeout(
                    tokio::time::Duration::from_secs(30),
                    ws.read_frame(),
                )
                .await
                {
                    Ok(Ok(msg)) => msg,
                    Ok(Err(e)) => {
                        println!("Error: {}", e);
                        let _ = ws.write_frame(Frame::close_raw(vec![].into())).await;
                        break;
                    }
                    Err(_) => {
                        println!("jetstream timeout");
                        break;
                    }
                };

                match msg.opcode {
                    OpCode::Text => {
                        let text = String::from_utf8_lossy(&msg.payload);
                        let event: Event =
                            serde_json::from_str(&text).expect("Failed to parse JSON");
                        last_time = Some(self.handle_event(event).await);
                    }
                    OpCode::Close => {
                        println!("got close");
                        break;
                    }
                    _ => {
                        println!("got other: {:?}", msg.opcode);
                        continue;
                    }
                }
            }
        }
    }

    async fn handle_event(&self, event: Event) -> u64 {
        let ret = event.time_us;

        if event.kind == "identity" {
            let handle = event.identity.as_ref().and_then(|i| i.handle.clone());

            // TODO: send to user's channels
            // TODO: update their rooms??
            self.users
                .alter(&event.did, |_, old| ircsky::User { handle, ..old });
        }

        if event.kind != "commit" {
            return ret;
        }

        if let Some(commit) = event.commit {
            if let Some(ref collection) = commit.collection {
                let record = commit.record.unwrap_or(serde_json::Value::Null);
                match collection.as_str() {
                    "social.psky.actor.profile" => {
                        let profile: Option<psky::Profile> = serde_json::from_value(record).ok();

                        // TODO: send to user's channels
                        self.users
                            .alter(&event.did, |_, old| ircsky::User { profile, ..old });
                    }
                    "social.psky.chat.room" => {
                        let room: Option<psky::Room> = serde_json::from_value(record).ok();
                        let room = match room {
                            Some(room) => room,
                            None => {
                                return ret;
                            }
                        };
                        let user = match self.get_user(&event.did).await {
                            Ok(user) => user.0.as_ref().to_owned(),
                            Err(_) => {
                                return ret;
                            }
                        };
                        let handle = match user.handle {
                            Some(handle) => handle,
                            None => {
                                return ret;
                            }
                        };
                        let mut entry = self
                            .channels
                            .entry(ircsky::ChannelUri(format!(
                                "at://{}/social.psky.chat.room/{}",
                                &event.did,
                                &commit.rkey.as_ref().unwrap()
                            )))
                            .or_insert_with(|| ircsky::Channel {
                                uri: ircsky::ChannelUri(format!(
                                    "at://{}/social.psky.chat.room/{}",
                                    &event.did,
                                    &commit.rkey.as_ref().unwrap()
                                )),
                                name: ircsky::ChannelName(format!("#{}@{}", &handle, &room.name)),
                                sender: tokio::sync::broadcast::channel(16).0,
                                users: HashSet::new(),
                                room: room.clone(),
                            });
                        if entry.room != room {
                            // TODO: handle room updates (what about name? actually what about user handle change?)
                            entry.room = room.clone();
                        }
                    }
                    "social.psky.chat.message" => {
                        let user = match self.get_user(&event.did).await {
                            Ok(user) => user.0.as_ref().to_owned(),
                            Err(_) => {
                                return ret;
                            }
                        };

                        let message: psky::Message = match serde_json::from_value(record) {
                            Ok(msg) => msg,
                            Err(_) => {
                                return ret;
                            }
                        };

                        self.channels
                            .alter(&message.room.clone(), |_, mut channel| {
                                channel.users.insert(user.did.clone()).then(|| {
                                    let _ = channel.sender.send(psky::PskyEvent::Join(
                                        user.clone(),
                                        channel.name.clone(),
                                    ));
                                });
                                let _ = channel.sender.send(psky::PskyEvent::PrivateMessage(
                                    user,
                                    message,
                                    channel.name.clone(),
                                ));
                                channel
                            });
                    }
                    _ => {}
                }
            }
        }

        ret
    }
}

use std::str::FromStr;

use anyhow::Result;
use atrium_api::types::TryIntoUnknown;
use irc_rust::{parsed::Parsed, Message};
use tokio::io::{AsyncRead, AsyncWrite};

use crate::{
    irc::{IrcClient, UserState},
    ircsky, psky,
};

impl<T> IrcClient<T>
where
    T: AsyncRead + AsyncWrite,
{
    pub async fn handle_privmsg(&mut self, message: Parsed<'_>) -> Result<()> {
        let nick = self.user.get_nick()?.to_owned();

        let channel_name = ircsky::ChannelName(
            message
                .param(0)
                .ok_or(anyhow::anyhow!("No channel given with PRIVMSG"))?
                .to_owned(),
        );

        let msg_line = message
            .params()
            .skip(1)
            .flatten()
            .copied()
            .collect::<Vec<_>>()
            .join(" ");
        let mut msg_line = msg_line.as_str();

        if message.trailing().is_some() {
            if !msg_line.is_empty() {
                return self
                    .send(
                        Message::builder("461")
                            .param(&nick)
                            .param("PRIVMSG")
                            .trailing("Not enough parameters")
                            .build(),
                    )
                    .await;
            }
            msg_line = message.trailing().unwrap();
        }

        if msg_line.is_empty() {
            return self
                .send(
                    Message::builder("461")
                        .param(&nick)
                        .param("PRIVMSG")
                        .trailing("Not enough parameters")
                        .build(),
                )
                .await;
        }

        let resolved = match self.ircsky.resolve_channel(&channel_name).await {
            Some(resolved) => resolved,
            None => {
                return self
                    .send(
                        Message::builder("404")
                            .param(&nick)
                            .param(&channel_name)
                            .trailing("Cannot send to channel")
                            .build(),
                    )
                    .await;
            }
        };

        if let UserState::LoggedIn(_, ref did, ref agent) = self.user {
            let record = atrium_api::com::atproto::repo::create_record::InputData {
                collection: atrium_api::types::string::Nsid::from_str("social.psky.chat.message")
                    .map_err(|e| anyhow::anyhow!(e))?,
                record: psky::Message {
                    r#type: "social.psky.chat.message".to_string(),
                    room: resolved,
                    content: msg_line.to_string(),
                    //  facets: None,
                }
                .try_into_unknown()?,
                repo: atrium_api::types::string::Did::from_str(did)
                    .map_err(|e| anyhow::anyhow!(e))?
                    .into(),
                rkey: None,
                swap_commit: None,
                validate: Some(false),
            };

            agent
                .api
                .com
                .atproto
                .repo
                .create_record(record.into())
                .await?;
        } else {
            self.send(
                Message::builder("404")
                    .param(&nick)
                    .param(channel_name)
                    .trailing("Cannot send to channel")
                    .build(),
            )
            .await?;
        }

        Ok(())
    }
}

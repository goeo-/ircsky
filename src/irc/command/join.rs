use anyhow::Result;
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
    pub async fn handle_join(&mut self, message: Parsed<'_>) -> Result<()> {
        let nick = self.user.get_nick()?.to_owned();
        let channels = message
            .param(0)
            .ok_or(anyhow::anyhow!("No channel given with JOIN"))?
            .split(',');

        for channel in channels {
            let channel_name = ircsky::ChannelName(channel.to_string());

            if self
                .channels
                .iter()
                .any(|(name, _)| name == channel_name.0.as_str())
            {
                return Ok(()); // Already in channel
            }

            let channel_uri = match self.ircsky.resolve_channel(&channel_name).await {
                Some(resolved) => resolved,
                None => {
                    return self
                        .send(
                            Message::builder("403")
                                .param(&nick)
                                .param(&channel_name)
                                .trailing("No such channel")
                                .build(),
                        )
                        .await;
                }
            };

            let mut channel = self
                .ircsky
                .channels
                .get_mut(&channel_uri)
                .ok_or(anyhow::anyhow!(
                    "resolve_channel should've inserted the channel",
                ))?;

            let has_topic = channel.room.topic.is_some();

            self.channels.push((
                channel_name.0.clone(),
                tokio_stream::wrappers::BroadcastStream::new(channel.sender.subscribe()),
            ));

            match self.user {
                UserState::LoggedIn(_, ref did, _) => {
                    let (user_, _) = self.ircsky.get_user(did).await?;
                    let user = user_.as_ref().clone();
                    drop(user_);

                    channel.users.insert(did.to_owned());

                    channel
                        .sender
                        .send(psky::PskyEvent::Join(user.clone(), channel.name.clone()))?;

                    drop(channel);
                    self.send(
                        Message::builder("JOIN")
                            .prefix(&nick, Some(user.did.as_str()), Some("the.atmosphere"))
                            .param(channel_name.clone())
                            .build(),
                    )
                    .await?;
                }
                UserState::LoggedOut(ref nick) => {
                    drop(channel);
                    self.send(
                        Message::builder("JOIN")
                            .prefix(nick, Some("logged-out"), Some("the.atmosphere"))
                            .param(channel_name.clone())
                            .build(),
                    )
                    .await?
                }
                _ => {
                    drop(channel);
                }
            }

            if has_topic {
                self.handle_topic(Message::from(format!("TOPIC {}", &channel_name)).parse()?)
                    .await?;
            }

            self.handle_names(Message::from(format!("NAMES {}", &channel_name)).parse()?)
                .await?;
        }
        Ok(())
    }
}

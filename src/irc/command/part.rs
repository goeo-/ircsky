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
    pub async fn handle_part(&mut self, message: Parsed<'_>) -> Result<()> {
        let channel_name = ircsky::ChannelName(
            message
                .param(0)
                .ok_or(anyhow::anyhow!("No channel given with PART"))?
                .to_owned(),
        );
        let nick = self.user.get_nick()?.to_owned();

        let resolved = match self.ircsky.resolve_channel(&channel_name).await {
            Some(resolved) => resolved,
            None => {
                return self
                    .send(
                        Message::builder("403")
                            .param(&nick)
                            .param(&channel_name.0)
                            .trailing("No such channel")
                            .build(),
                    )
                    .await;
            }
        };

        dbg!(&channel_name);
        dbg!(&self.channels);

        match self
            .channels
            .iter()
            .position(|(name, _)| name == channel_name.0.as_str())
        {
            Some(idx) => {
                self.channels.remove(idx);
            }
            None => {
                return self
                    .send(
                        Message::builder("442")
                            .param(&nick)
                            .param(&channel_name)
                            .trailing("You're not on that channel")
                            .build(),
                    )
                    .await;
            }
        }

        match self.user {
            UserState::LoggedIn(_, ref did, _) => {
                let (user_, _) = self.ircsky.get_user(did).await?;
                let user = user_.as_ref().clone();
                drop(user_);

                self.ircsky.channels.alter(&resolved, |_, mut channel| {
                    channel.users.remove(did);
                    _ = channel
                        .sender
                        .send(psky::PskyEvent::Part(user.clone(), channel_name.to_owned()));
                    channel
                });

                self.send(
                    Message::builder("PART")
                        .prefix(nick, Some(user.did), Some("the.atmosphere"))
                        .param(channel_name)
                        .build(),
                )
                .await?
            }
            UserState::LoggedOut(ref nick) => {
                self.send(
                    Message::builder("PART")
                        .prefix(nick, Some("logged-out"), Some("the.atmosphere"))
                        .param(channel_name)
                        .build(),
                )
                .await?
            }
            _ => {}
        }

        Ok(())
    }
}

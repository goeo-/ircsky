use anyhow::Result;
use irc_rust::{parsed::Parsed, Message};
use tokio::io::{AsyncRead, AsyncWrite};

use crate::{
    atproto,
    irc::IrcClient,
    ircsky::{ChannelName, User},
};

impl<T> IrcClient<T>
where
    T: AsyncRead + AsyncWrite,
{
    pub async fn handle_who(&mut self, message: Parsed<'_>) -> Result<()> {
        let mask = message
            .param(0)
            .ok_or(anyhow::anyhow!("No first argument given with WHO"))?;
        let nick = self.user.get_nick()?.to_owned();

        if mask.starts_with('#') {
            // channel
            let channel_name = ChannelName(mask.to_string());
            let channel_uri = match self.ircsky.resolve_channel(&channel_name).await {
                Some(channel) => channel,
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

            let channel = self.ircsky.channels.get(&channel_uri);
            if channel.is_none() {
                drop(channel);
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
            let channel = channel.unwrap();
            let users = channel
                .users
                .iter()
                .filter_map(|did| self.ircsky.users.get(did))
                .map(|user| user.value().clone())
                .collect::<Vec<_>>();
            drop(channel);
            for user in users {
                self.send_who(&nick, mask, user).await?;
            }
        } else {
            // user
            let did = atproto::resolve_handle(mask).await?.to_string();
            let user = self.ircsky.get_user(&did).await?.0.as_ref().to_owned();

            self.send_who(&nick, "*", user).await?;
            self.send_endofwho(&nick, mask).await?;
        }

        Ok(())
    }

    async fn send_who(&mut self, nick: &str, mask: &str, user: User) -> Result<()> {
        let handle = match user.handle {
            Some(handle) => handle,
            None => {
                return self
                    .send(
                        Message::builder("401")
                            .param(nick)
                            .param(mask)
                            .trailing("No such user")
                            .build(),
                    )
                    .await;
            }
        };

        let realname = match user.profile {
            Some(profile) => match profile.nickname {
                Some(nickname) => nickname,
                None => handle.clone(),
            },
            None => handle.clone(),
        };

        self.send(
            Message::builder("352")
                .param(nick)
                .param(mask)
                .param(user.did)
                .param("the.atmosphere")
                .param("ircsky")
                .param(handle)
                .param("H")
                .trailing(format!("0 {}", realname))
                .build(),
        )
        .await
    }

    async fn send_endofwho(&mut self, nick: &str, mask: &str) -> Result<()> {
        self.send(
            Message::builder("315")
                .param(nick)
                .param(mask)
                .trailing("End of WHO list")
                .build(),
        )
        .await
    }
}

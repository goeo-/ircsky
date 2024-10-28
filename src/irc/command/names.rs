use anyhow::Result;
use irc_rust::{parsed::Parsed, Message};
use tokio::io::{AsyncRead, AsyncWrite};

use crate::{irc::IrcClient, ircsky::ChannelName};

impl<T> IrcClient<T>
where
    T: AsyncRead + AsyncWrite,
{
    pub async fn handle_names(&mut self, message: Parsed<'_>) -> Result<()> {
        let nick = self.user.get_nick()?.to_owned();
        let channels = message
            .param(0)
            .ok_or(anyhow::anyhow!("No channel given with NAMES"))?
            .split(',');

        for channel in channels {
            let channel_name = ChannelName(channel.to_string());
            let uri = match self.ircsky.resolve_channel(&channel_name).await {
                Some(uri) => uri,
                None => {
                    self.send_end_of_names(&channel_name).await?;
                    continue;
                }
            };
            let channel = self.ircsky.channels.get(&uri);
            let users = match channel {
                Some(ref channel) => {
                    let mut ret = Vec::new();
                    for user in &channel.users {
                        if let Some(user_) = self.ircsky.users.get(user) {
                            if let Some(handle) = user_.handle.as_ref() {
                                ret.push(handle.to_string());
                            }
                        }
                    }
                    ret
                }
                None => {
                    drop(channel);
                    self.send_end_of_names(&channel_name).await?;
                    continue;
                }
            };
            drop(channel);

            for chunk in users.chunks(12) {
                let user_chunk = chunk.join(" ");
                self.send(
                    Message::builder("353")
                        .param(&nick)
                        .param("=")
                        .param(&channel_name)
                        .trailing(&user_chunk)
                        .build(),
                )
                .await?;
            }
            self.send_end_of_names(&channel_name).await?;
        }

        Ok(())
    }

    async fn send_end_of_names(&mut self, channel: impl ToString) -> Result<()> {
        let nick = self.user.get_nick()?.to_owned();

        self.send(
            Message::builder("366")
                .param(&nick)
                .param(channel)
                .trailing("End of /NAMES list")
                .build(),
        )
        .await
        .unwrap();

        Ok(())
    }
}

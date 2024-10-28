use anyhow::Result;
use irc_rust::{parsed::Parsed, Message};
use tokio::io::{AsyncRead, AsyncWrite};

use crate::irc::IrcClient;
use crate::ircsky::ChannelName;

impl<T> IrcClient<T>
where
    T: AsyncRead + AsyncWrite,
{
    pub async fn handle_topic(&mut self, message: Parsed<'_>) -> Result<()> {
        let nick = self.user.get_nick()?.to_owned();

        if message.param(1).is_some() {
            self.send(
                Message::builder("482")
                    .param(&nick)
                    .param(message.param(0).unwrap())
                    .trailing("You're not channel operator")
                    .build(),
            )
            .await?;
        }

        let channel_name = match message.param(0) {
            Some(channel) => ChannelName(channel.to_string()),
            None => {
                return self
                    .send(
                        Message::builder("461")
                            .param(&nick)
                            .param("TOPIC")
                            .trailing("Not enough parameters")
                            .build(),
                    )
                    .await;
            }
        };

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

        let channel = self
            .ircsky
            .channels
            .get(&channel_uri)
            .ok_or(anyhow::anyhow!(
                "resolve_channel should've inserted the channel"
            ))?;

        let topic = channel.room.topic.clone();
        drop(channel);

        match topic {
            Some(topic) => {
                self.send(
                    Message::builder("332")
                        .param(&nick)
                        .param(&channel_name)
                        .trailing(&topic)
                        .build(),
                )
                .await?;
            }
            None => {
                self.send(
                    Message::builder("331")
                        .param(&nick)
                        .param(&channel_name)
                        .trailing("No topic is set")
                        .build(),
                )
                .await?;
            }
        }
        Ok(())
    }
}

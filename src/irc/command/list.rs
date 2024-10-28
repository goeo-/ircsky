use anyhow::Result;
use irc_rust::{parsed::Parsed, Message};
use tokio::io::{AsyncRead, AsyncWrite};

use crate::irc::IrcClient;

impl<T> IrcClient<T>
where
    T: AsyncRead + AsyncWrite,
{
    pub async fn handle_list(&mut self, _message: Parsed<'_>) -> Result<()> {
        let nick = self.user.get_nick()?.to_owned();

        let channels = self
            .ircsky
            .channels
            .iter()
            .map(|channel| {
                (
                    channel.name.clone(),
                    channel.users.len(),
                    channel.room.topic.clone(),
                )
            })
            .collect::<Vec<_>>();

        self.send(
            Message::builder("321")
                .param(&nick)
                .param("Channel")
                .trailing("Users  Name")
                .build(),
        )
        .await?;

        for (name, count, topic) in channels {
            let mut message = Message::builder("322")
                .param(&nick)
                .param(&name.0)
                .param(count.to_string());

            if let Some(topic) = topic {
                message = message.trailing(topic);
            }

            self.send(message.build()).await?;
        }

        self.send(
            Message::builder("323")
                .param(&nick)
                .trailing("End of /LIST")
                .build(),
        )
        .await
    }
}

use anyhow::Result;
use irc_rust::{parsed::Parsed, Message};
use tokio::io::{AsyncRead, AsyncWrite};

use crate::{irc::IrcClient, ircsky::ChannelName};

impl<T> IrcClient<T>
where
    T: AsyncRead + AsyncWrite,
{
    pub async fn handle_mode(&mut self, message: Parsed<'_>) -> Result<()> {
        let nick = self.user.get_nick()?.to_owned();
        let mode_of = message
            .param(0)
            .ok_or(anyhow::anyhow!("No first parameter given for MODE"))?;

        if mode_of.starts_with('#') {
            if message.param(1).is_some() {
                self.send(
                    Message::builder("482")
                        .param(&nick)
                        .param(mode_of)
                        .trailing("You're not channel operator")
                        .build(),
                )
                .await
            } else if self
                .ircsky
                .resolve_channel(&ChannelName(mode_of.to_string()))
                .await
                .is_some()
            {
                self.send(
                    Message::builder("324")
                        .param(&nick)
                        .param(mode_of)
                        .param("+nrt")
                        .build(),
                )
                .await
            } else {
                self.send(
                    Message::builder("403")
                        .param(&nick)
                        .param(mode_of)
                        .trailing("No such channel")
                        .build(),
                )
                .await
            }
        } else if message.param(1).is_some() {
            self.send(
                Message::builder("501")
                    .param(&nick)
                    .trailing("Unknown MODE flag")
                    .build(),
            )
            .await
        } else {
            self.send(
                Message::builder("502")
                    .param(&nick)
                    .trailing("Cant change mode for other users")
                    .build(),
            )
            .await
        }
    }
}

use anyhow::Result;
use irc_rust::{parsed::Parsed, Message};
use tokio::io::{AsyncRead, AsyncWrite};

use crate::irc::IrcClient;

impl<T> IrcClient<T>
where
    T: AsyncRead + AsyncWrite,
{
    pub async fn handle_motd(&mut self, _message: Parsed<'_>) -> Result<()> {
        let nick = self.user.get_nick()?.to_owned();

        match self.ircsky.config.irc.motd() {
            Some(motd) => {
                self.send(
                    Message::builder("375")
                        .param(&nick)
                        .trailing("- ircsky Message of the day - ")
                        .build(),
                )
                .await?;

                for line in motd.lines() {
                    self.send(Message::builder("372").param(&nick).trailing(line).build())
                        .await?;
                }

                self.send(
                    Message::builder("376")
                        .param(&nick)
                        .trailing("End of /MOTD command.")
                        .build(),
                )
                .await?;
            }
            None => {
                self.send(
                    Message::builder("422")
                        .param(&nick)
                        .trailing("MOTD File is missing")
                        .build(),
                )
                .await?;
            }
        }

        Ok(())
    }
}

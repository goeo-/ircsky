use anyhow::{Context, Result};
use irc_rust::{parsed::Parsed, Message};
use tokio::io::{AsyncRead, AsyncWrite};

use crate::irc::{IrcClient, ParamMaybe};

impl<T> IrcClient<T>
where
    T: AsyncRead + AsyncWrite,
{
    pub async fn handle_other(&mut self, message: Parsed<'_>) -> Result<()> {
        let nick = self
            .user
            .get_nick()
            .context("Unknown command before registration")?;

        self.send(
            Message::builder("421")
                .param(nick)
                .param_maybe(message.command())
                .trailing("Unknown command")
                .build(),
        )
        .await
    }
}

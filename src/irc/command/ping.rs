use anyhow::Result;
use irc_rust::{parsed::Parsed, Message};
use tokio::io::{AsyncRead, AsyncWrite};

use crate::irc::{IrcClient, ParamMaybe};

impl<T> IrcClient<T>
where
    T: AsyncRead + AsyncWrite,
{
    pub async fn handle_ping(&mut self, message: Parsed<'_>) -> Result<()> {
        self.send(
            Message::builder("PONG")
                .param("ircsky")
                .param_maybe(message.param(0))
                .build(),
        )
        .await
    }
}

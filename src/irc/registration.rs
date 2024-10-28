use std::str::FromStr;

use anyhow::Result;
use irc_rust::Message;
use tokio::io::{AsyncRead, AsyncWrite};

use crate::irc::IrcClient;

impl<T> IrcClient<T>
where
    T: AsyncRead + AsyncWrite,
{
    pub async fn register_user(&mut self) -> Result<()> {
        let nick = self.user.get_nick()?.to_owned();
        let did = self.user.did().to_owned().unwrap_or("logged-out");

        self.send(
            Message::builder("001")
                .param(&nick)
                .trailing(format!("welcome to ircsky, {nick}!{did}@the.atmosphere"))
                .build(),
        )
        .await?;

        self.send(
            Message::builder("002")
                .param(&nick)
                .trailing("you're connected to ircsky")
                .build(),
        )
        .await?;

        self.send(
            Message::builder("003")
                .param(&nick)
                .trailing("ircsky was made late 2024")
                .build(),
        )
        .await?;

        self.send(
            Message::builder("004")
                .param(&nick)
                .param("ircsky")
                .param("1")
                .param("+")
                .param("+t")
                .build(),
        )
        .await?;

        self.send(
            Message::builder("005")
                .param(&nick)
                .param("IRCSKY")
                .trailing("are supported by this server")
                .build(),
        )
        .await?;

        // MAY then send other numerics and messages
        // TODO: SHOULD then respond as though the client sent the LUSERS command

        // MUST then respond as though the client sent it the MOTD command
        self.handle_motd(Message::from_str("MOTD")?.parse()?)
            .await?;

        self.handle_join(Message::from_str("JOIN #general@psky.social")?.parse()?)
            .await?;

        Ok(())
    }
}

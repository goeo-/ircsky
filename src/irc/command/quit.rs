use anyhow::Result;
use irc_rust::parsed::Parsed;
use tokio::io::{AsyncRead, AsyncWrite};

use crate::irc::IrcClient;

impl<T> IrcClient<T>
where
    T: AsyncRead + AsyncWrite,
{
    pub async fn handle_quit(&mut self, _message: Parsed<'_>) -> Result<()> {
        // TODO: send PART messages if was LoggedIn
        self.stop().await;
        Ok(())
    }
}

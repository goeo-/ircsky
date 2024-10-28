use anyhow::Result;
use irc_rust::parsed::Parsed;
use tokio::io::{AsyncRead, AsyncWrite};

use crate::irc::{IrcClient, UserState};

impl<T> IrcClient<T>
where
    T: AsyncRead + AsyncWrite,
{
    pub async fn handle_pass(&mut self, message: Parsed<'_>) -> Result<()> {
        if !matches!(self.user, UserState::New) {
            anyhow::bail!("Cannot PASS multiple times, or after NICK")
        }

        let pass = message
            .param(0)
            .or(message.trailing())
            .ok_or(anyhow::anyhow!("No password given with PASS"))?;

        self.user = UserState::Pass(pass.to_string());
        Ok(())
    }
}

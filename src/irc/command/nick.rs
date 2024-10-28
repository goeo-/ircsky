use anyhow::Result;
use irc_rust::{parsed::Parsed, Message};
use tokio::io::{AsyncRead, AsyncWrite};

use atrium_api::agent::{store::MemorySessionStore, AtpAgent};
use atrium_xrpc_client::reqwest::ReqwestClient;
use tokio_stream::wrappers::BroadcastStream;

use crate::atproto;
use crate::irc::{IrcClient, UserState};

impl<T> IrcClient<T>
where
    T: AsyncRead + AsyncWrite,
{
    pub async fn handle_nick(&mut self, message: Parsed<'_>) -> Result<()> {
        let nick = message
            .param(0)
            .or(message.trailing())
            .ok_or(anyhow::anyhow!("No nickname given with NICK"))?;

        match &self.user {
            UserState::New => {
                println!("no PASS, got NICK {nick}, creating LoggedOut user");

                self.user = UserState::LoggedOut(nick.to_string());
                self.register_user().await?;
                self.send(
                    Message::builder("NOTICE")
                        .prefix("ircsky", None::<String>, None::<String>)
                        .param(nick)
                        .trailing("Logged in as a guest, as no PASS was given. You are invisible to other users.")
                        .build(),
                )
                .await
            }
            UserState::Pass(password) => {
                println!("PASS: {password}, got NICK {nick}");

                let (did, auth_server) = atproto::get_did_and_auth_endpoint(nick).await?;
                let agent = AtpAgent::new(
                    ReqwestClient::new(&auth_server),
                    MemorySessionStore::default(),
                );
                let result = agent.login(&nick, &password).await?;

                println!("{:?}", result);

                if did != result.did.as_str() {
                    anyhow::bail!("DID mismatch");
                }

                let (tx, rx) = tokio::sync::broadcast::channel(16);

                self.channels
                    .push(("dm".to_string(), BroadcastStream::new(rx)));
                self.user = UserState::LoggedIn(nick.to_string(), did.clone(), agent);

                self.ircsky.get_user(&did).await?;
                self.ircsky.users.alter(&did, |_, mut user| {
                    user.sender = Some(tx);
                    user
                });

                self.register_user().await
            }
            _ => {
                self.send(
                    Message::builder("433")
                        .param(self.user.get_nick()?)
                        .trailing("Can't change nickname")
                        .build(),
                )
                .await
            }
        }
    }
}

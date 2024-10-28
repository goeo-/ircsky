use anyhow::Result;
use irc_rust::{parsed::Parsed, Message};
use tokio::io::{AsyncRead, AsyncWrite};

use crate::irc::{CapState, IrcClient};

impl<T> IrcClient<T>
where
    T: AsyncRead + AsyncWrite,
{
    pub async fn handle_cap(&mut self, message: Parsed<'_>) -> Result<()> {
        let subcommand = message
            .param(0)
            .ok_or_else(|| anyhow::anyhow!("Missing subcommand"))?;

        match self.cap {
            CapState::New => {
                if subcommand != "LS" && subcommand != "REQ" {
                    anyhow::bail!("First CAP subcommand must be LS or REQ");
                }
                self.cap = CapState::Negotiating(Vec::new());
            }
            CapState::Capabilities(_) => {
                if subcommand != "LIST" {
                    anyhow::bail!("You can only send CAP LIST after CAP END");
                }
            }
            _ => {}
        }

        match subcommand {
            "LS" => {
                self.send(
                    Message::builder("CAP")
                        .param("*")
                        .param("LS")
                        .trailing("echo-message")
                        .build(),
                )
                .await
            }
            "LIST" => {
                self.send(
                    Message::builder("CAP")
                        .param("*")
                        .param("LIST")
                        .trailing(
                            self.cap
                                .capabilities()
                                .ok_or(anyhow::anyhow!("Couldn't list capabilities"))?
                                .join(" "),
                        )
                        .build(),
                )
                .await
            }
            "REQ" => {
                let requested = message
                    .trailing()
                    .ok_or_else(|| anyhow::anyhow!("Missing requested capability"))?
                    .split(' ');

                let mut ack = Vec::new();
                let mut nak = Vec::new();

                for capability in requested {
                    match capability {
                        "echo-message" => ack.push(capability.to_string()),
                        _ => nak.push(capability.to_string()),
                    }
                }

                if !ack.is_empty() {
                    self.cap.add_capabilities(ack.clone())?;
                    self.send(
                        Message::builder("CAP")
                            .param(self.user.nick().unwrap())
                            .param("ACK")
                            .param(ack.join(" "))
                            .build(),
                    )
                    .await?;
                }

                if !nak.is_empty() {
                    self.send(
                        Message::builder("CAP")
                            .param(self.user.nick().unwrap())
                            .param("NAK")
                            .param(ack.join(" "))
                            .build(),
                    )
                    .await?;
                }

                Ok(())
            }
            "END" => match self.cap {
                CapState::Negotiating(ref mut caps) => {
                    self.cap = CapState::Capabilities(std::mem::take(caps));
                    Ok(())
                }
                _ => anyhow::bail!("CAP END without CAP LS/REQ"),
            },
            _ => Ok(()),
        }
    }
}

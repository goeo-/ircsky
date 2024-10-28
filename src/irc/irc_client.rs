use anyhow::Result;
use irc_rust::Message;
use tokio::io::{
    AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader, ReadHalf, WriteHalf,
};
use tokio_stream::{StreamExt, StreamMap};

use atrium_api::agent::{store::MemorySessionStore, AtpAgent};
use atrium_xrpc_client::reqwest::ReqwestClient;

use crate::psky::PskyEvent;
use crate::Ircsky;

pub enum UserState {
    New,
    Pass(String),
    LoggedIn(String, String, AtpAgent<MemorySessionStore, ReqwestClient>),
    LoggedOut(String),
}

impl UserState {
    pub fn nick(&self) -> Option<&str> {
        match self {
            UserState::LoggedIn(nick, _, _) => Some(nick.as_str()),
            UserState::LoggedOut(nick) => Some(nick.as_str()),
            _ => None,
        }
    }

    pub fn get_nick(&self) -> Result<&str> {
        self.nick().ok_or(anyhow::anyhow!("No nickname"))
    }

    pub fn did(&self) -> Option<&str> {
        match self {
            UserState::LoggedIn(_, did, _) => Some(did.as_str()),
            _ => None,
        }
    }
}

pub enum CapState {
    New,
    Negotiating(Vec<String>),
    Capabilities(Vec<String>),
}

impl CapState {
    pub fn capabilities(&self) -> Option<&[String]> {
        match self {
            CapState::Negotiating(caps) => Some(caps.as_slice()),
            CapState::Capabilities(caps) => Some(caps.as_slice()),
            _ => None,
        }
    }
    pub fn add_capabilities(&mut self, caps: Vec<String>) -> Result<()> {
        match self {
            CapState::Negotiating(capss) => {
                capss.extend(caps);
                Ok(())
            }
            _ => {
                anyhow::bail!("Can't add capabilities after negotiation")
            }
        }
    }
    pub fn has_capability(&self, cap: &str) -> bool {
        match self {
            CapState::Capabilities(caps) | CapState::Negotiating(caps) => {
                caps.contains(&cap.to_string())
            }
            _ => false,
        }
    }
}

pub struct IrcClient<T>
where
    T: AsyncRead + AsyncWrite,
{
    pub user: UserState,
    pub cap: CapState,
    read: BufReader<ReadHalf<T>>,
    write: WriteHalf<T>,
    pub ircsky: Ircsky,
    line_buffer: Vec<u8>,
    empty_lines: usize,
    pub channels: Vec<(String, tokio_stream::wrappers::BroadcastStream<PskyEvent>)>,
}

impl<T> IrcClient<T>
where
    T: AsyncRead + AsyncWrite,
{
    fn new(ircsky: Ircsky, socket: T) -> Self {
        let (read, write) = tokio::io::split(socket);
        let read = BufReader::new(read);

        Self {
            user: UserState::New,
            cap: CapState::New,
            read,
            write,
            ircsky,
            line_buffer: Vec::new(),
            empty_lines: 0,
            channels: vec![],
        }
    }

    async fn start(mut self) {
        if let Err(e) = self._start().await {
            let message = Message::builder("ERROR").trailing(e).build();
            _ = self.write(message.to_string().as_bytes()).await;
            _ = self.write.shutdown().await;
        }
    }

    async fn _start(&mut self) -> Result<()> {
        loop {
            let mut map =
                StreamMap::from_iter(self.channels.iter_mut().map(|(n, c)| (n.as_str(), c)));

            tokio::select! {
                _ = self.read.read_until(b'\n', &mut self.line_buffer) => {
                    drop(map);
                    self.handle_line().await?;
                    self.line_buffer.truncate(0);
                }

                Some((_, event)) = map.next() => {
                    drop(map);
                    self.handle_event(event?).await?;
                }
            }
        }
    }

    pub async fn stop(&mut self) {
        _ = self.write.shutdown().await;
    }

    async fn write(&mut self, data: &[u8]) -> Result<()> {
        self.write.write_all(data).await?;
        self.write.write_all(b"\r\n").await?;

        Ok(())
    }

    pub async fn send(&mut self, message: Message) -> Result<()> {
        self.write(message.to_string().as_bytes()).await
    }

    fn received_empty(&mut self) -> Result<()> {
        self.empty_lines += 1;

        if self.empty_lines > 10 {
            anyhow::bail!("ircsky speaks IRC");
        }

        Ok(())
    }

    async fn handle_line(&mut self) -> Result<()> {
        let line = String::from_utf8_lossy(&self.line_buffer);
        let line = line.trim_end_matches(&['\r', '\n']).trim();

        if line.is_empty() {
            return self.received_empty();
        }

        let message = Message::from(line);
        let message = match message.parse() {
            Ok(message) => message,
            Err(e) => {
                anyhow::bail!("Could not parse IRC message: {:?}", e);
            }
        };

        dbg!(&message);

        let command = message.command().unwrap_or("NOCOMMAND");

        match command.to_uppercase().as_str() {
            "CAP" => self.handle_cap(message).await,
            "JOIN" => self.handle_join(message).await,
            "LIST" => self.handle_list(message).await,
            "MODE" => self.handle_mode(message).await,
            "NAMES" => self.handle_names(message).await,
            "NICK" => self.handle_nick(message).await,
            "PART" => self.handle_part(message).await,
            "PASS" => self.handle_pass(message).await,
            "PING" => self.handle_ping(message).await,
            "PONG" => Ok(()),
            "PRIVMSG" => self.handle_privmsg(message).await,
            "QUIT" => self.handle_quit(message).await,
            "TOPIC" => self.handle_topic(message).await,
            "USER" => Ok(()),
            "WHO" => self.handle_who(message).await,
            _ => self.handle_other(message).await,
        }
    }
    async fn handle_event(&mut self, event: PskyEvent) -> Result<()> {
        match event {
            PskyEvent::PrivateMessage(user, message, room) => {
                if let Some(did) = self.user.did() {
                    if !self.cap.has_capability("echo-message") && user.did == did {
                        return Ok(());
                    }
                }

                self.send(
                    Message::builder("PRIVMSG")
                        .prefix(
                            user.handle.as_ref().ok_or(std::fmt::Error)?,
                            Some(&user.did),
                            Some("the.atmosphere"),
                        )
                        .param(&room)
                        .trailing(message.content.as_str())
                        .build(),
                )
                .await?;
            }
            PskyEvent::Join(user, room) => {
                if let Some(did) = self.user.did() {
                    if user.did == did {
                        return Ok(());
                    }
                }
                self.send(
                    Message::builder("JOIN")
                        .prefix(
                            user.handle.as_ref().ok_or(std::fmt::Error)?,
                            Some(&user.did),
                            Some("the.atmosphere"),
                        )
                        .param(&room)
                        .build(),
                )
                .await?;
            }
            PskyEvent::Part(user, room) => {
                if let Some(did) = self.user.did() {
                    if user.did == did {
                        return Ok(());
                    }
                }
                self.send(
                    Message::builder("PART")
                        .prefix(
                            user.handle.as_ref().ok_or(std::fmt::Error)?,
                            Some(&user.did),
                            Some("the.atmosphere"),
                        )
                        .param(&room)
                        .build(),
                )
                .await?;
            }
        }
        Ok(())
    }
}

impl Ircsky {
    pub async fn start_irc_server(self) -> Result<()> {
        let config = &self.config.irc;

        let listener = tokio::net::TcpListener::bind((config.host.as_str(), config.port)).await?;

        let tls_acceptor = config.tls.acceptor()?;

        // TODO: respect allowlist/denylist
        // TODO: fewer clones
        // TODO: channel mode for allowlist/denylist
        // TODO: channel creation, topic and mode setting, etc
        // TODO: facets

        loop {
            let (socket, _) = listener.accept().await.unwrap();
            let tls_acceptor = tls_acceptor.clone();
            let ircsky = self.clone();

            tokio::spawn(async move {
                if let Some(tls_acceptor) = tls_acceptor {
                    let tls_socket = tls_acceptor.accept(socket).await.unwrap();
                    IrcClient::new(ircsky, tls_socket).start().await;
                } else {
                    IrcClient::new(ircsky, socket).start().await;
                }
            });
        }
    }
}

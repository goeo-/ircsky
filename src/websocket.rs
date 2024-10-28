use std::future::Future;
use std::sync::Arc;

use anyhow::{Context, Result};
use bytes::Bytes;
use fastwebsockets::{FragmentCollector, Frame, WebSocketError};
use http_body_util::Empty;
use tokio::net::TcpStream;
use tokio_rustls::rustls::ClientConfig;
use tokio_rustls::TlsConnector;

struct SpawnExecutor;

impl<Fut> hyper::rt::Executor<Fut> for SpawnExecutor
where
    Fut: Future + Send + 'static,
    Fut::Output: Send + 'static,
{
    fn execute(&self, fut: Fut) {
        tokio::spawn(fut);
    }
}

fn tls_connector() -> Result<TlsConnector> {
    let root_store = tokio_rustls::rustls::RootCertStore {
        roots: webpki_roots::TLS_SERVER_ROOTS.into(),
    };

    let config = ClientConfig::builder_with_provider(Arc::new(rustls_rustcrypto::provider()))
        .with_safe_default_protocol_versions()?
        .with_root_certificates(root_store)
        .with_no_client_auth();

    Ok(TlsConnector::from(Arc::new(config)))
}

pub async fn connect(domain: &str, port: u16, path: &str) -> Result<impl FrameStream> {
    let path = path.trim_start_matches('/');

    let tcp_stream = TcpStream::connect(&format!("{}:{}", domain, port)).await?;
    let tls_connector = tls_connector().unwrap();

    let tls_stream = tls_connector
        .connect(
            domain
                .to_owned()
                .try_into()
                .context("Converting domain to ServerName failed")?,
            tcp_stream,
        )
        .await?;

    let req = hyper::Request::builder()
        .method("GET")
        .uri(format!("wss://{}/{}", &domain, &path))
        .header("Host", domain)
        .header(hyper::header::UPGRADE, "websocket")
        .header(hyper::header::CONNECTION, "upgrade")
        .header(
            hyper::header::USER_AGENT,
            "ircsky - ircs://ircsky.genco.me - dms open @genco.me",
        )
        .header(
            "Sec-WebSocket-Key",
            fastwebsockets::handshake::generate_key(),
        )
        .header("Sec-WebSocket-Version", "13")
        .body(Empty::<Bytes>::new())?;

    /*
    dbg!(&req);

    let (mut sender, conn) =
        hyper::client::conn::http1::handshake(TokioIo::new(tls_stream)).await?;

    let fut = Box::pin(async move {
        if let Err(e) = conn.with_upgrades().await {
            eprintln!("Error polling connection: {}", e);
        }
    });
    SpawnExecutor.execute(fut);

    let response = sender.send_request(req).await?;
    dbg!(&response);
    */

    let (ws, _) = fastwebsockets::handshake::client(&SpawnExecutor, req, tls_stream).await?;
    Ok(FragmentCollector::new(ws))
}

pub trait FrameStream {
    fn read_frame(
        &mut self,
    ) -> impl std::future::Future<Output = Result<Frame, WebSocketError>> + Send;
    fn write_frame(
        &mut self,
        frame: Frame,
    ) -> impl std::future::Future<Output = Result<(), WebSocketError>> + Send;
}

impl FrameStream for FragmentCollector<hyper_util::rt::TokioIo<hyper::upgrade::Upgraded>> {
    async fn read_frame(&mut self) -> Result<Frame, WebSocketError> {
        self.read_frame().await
    }

    async fn write_frame<'f>(&mut self, frame: Frame<'f>) -> Result<(), WebSocketError> {
        self.write_frame(frame).await
    }
}

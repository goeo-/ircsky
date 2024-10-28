use anyhow::Context;
use rustls_pemfile::{certs, private_key};
use rustls_pki_types::{CertificateDer, PrivateKeyDer};
use serde_aux::field_attributes::deserialize_number_from_string;
use std::fs::File;
use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use tokio_rustls::{rustls, TlsAcceptor};

#[derive(serde::Deserialize, Clone, Debug)]
pub struct Settings {
    pub jetstream: JetstreamSettings,
    pub psky: PskySettings,
    pub irc: IrcSettings,
}

pub fn get_config() -> Result<Settings, config::ConfigError> {
    let base_path = std::env::current_dir().expect("Failed to determine the current directory");

    let settings = config::Config::builder()
        .add_source(config::File::from(base_path.join("config.yaml")))
        .add_source(
            config::Environment::with_prefix("IRCSKY")
                .prefix_separator("_")
                .separator("__"),
        )
        .build()?;
    settings.try_deserialize::<Settings>()
}

#[derive(serde::Deserialize, Clone, Debug)]
pub struct JetstreamSettings {
    pub host: String,
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub port: u16,
}

#[derive(serde::Deserialize, Clone, Debug)]
pub struct PskySettings {
    pub general: String,
}

#[derive(serde::Deserialize, Clone, Debug)]
pub struct IrcSettings {
    pub host: String,
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub port: u16,
    pub tls: TlsSettings,
    pub motd: Option<String>,
}

impl IrcSettings {
    pub fn motd(&self) -> Option<String> {
        match self.motd {
            Some(ref motd) => {
                let path = PathBuf::from(motd);
                match std::fs::read_to_string(&path) {
                    Ok(motd) => Some(motd),
                    Err(_) => Some(motd.clone()),
                }
            }
            None => None,
        }
    }
}

#[derive(serde::Deserialize, Clone, Debug)]
pub struct TlsSettings {
    pub enabled: bool,
    pub certs: Option<PathBuf>,
    pub key: Option<PathBuf>,
}

fn load_certs(path: &PathBuf) -> io::Result<Vec<CertificateDer<'static>>> {
    certs(&mut io::BufReader::new(File::open(path)?)).collect()
}

fn load_key(path: &PathBuf) -> io::Result<PrivateKeyDer<'static>> {
    private_key(&mut io::BufReader::new(File::open(path)?))
        .unwrap()
        .ok_or(io::Error::new(
            io::ErrorKind::Other,
            "no private key found".to_string(),
        ))
}

impl TlsSettings {
    pub fn acceptor(&self) -> anyhow::Result<Option<TlsAcceptor>> {
        if self.enabled {
            let certs = load_certs(
                self.certs
                    .as_ref()
                    .context("TLS enabled but no certs given")?,
            )?;
            let key = load_key(self.key.as_ref().context("TLS enabled but no key given")?)?;

            let config = rustls::ServerConfig::builder_with_provider(Arc::new(
                rustls_rustcrypto::provider(),
            ))
            .with_safe_default_protocol_versions()?
            .with_no_client_auth()
            .with_single_cert(certs, key)?;

            Ok(Some(TlsAcceptor::from(Arc::new(config))))
        } else {
            Ok(None)
        }
    }
}

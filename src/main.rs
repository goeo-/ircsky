use ircsky::{get_config, Ircsky};

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let config = get_config().expect("Failed to read configuration.");
    Ircsky::new(config).run().await
}

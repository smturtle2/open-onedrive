mod app;
mod bus;

use anyhow::Result;
use bus::{DBUS_PATH, DBUS_SERVICE, OpenOneDriveBus};
use clap::Parser;
use tracing::info;
use zbus::Connection;

#[derive(Debug, Parser)]
#[command(name = "openonedrived")]
#[command(about = "open-onedrive background daemon")]
struct Args {
    #[arg(long, help = "Print resolved config and exit")]
    print_config: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "openonedrived=info".into()),
        )
        .init();

    let args = Args::parse();
    if args.print_config {
        let paths = openonedrive_config::ProjectPaths::discover()?;
        let config = openonedrive_config::AppConfig::load(&paths)?;
        println!("{}", toml::to_string_pretty(&config)?);
        return Ok(());
    }

    let app = app::OpenOneDriveApp::load().await?;

    let connection = Connection::session().await?;
    connection.request_name(DBUS_SERVICE).await?;
    connection
        .object_server()
        .at(DBUS_PATH, OpenOneDriveBus::new(app))
        .await?;

    info!("openonedrived ready on {DBUS_SERVICE}");
    tokio::signal::ctrl_c().await?;
    Ok(())
}

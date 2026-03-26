mod app;
mod bus;

use anyhow::Result;
use app::OpenOneDriveApp;
use bus::{DBUS_PATH, DBUS_SERVICE, OpenOneDriveBus};
use clap::Parser;
use openonedrive_rclone_backend::BackendEvent;
use tracing::{info, warn};
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

    let app = OpenOneDriveApp::load().await?;
    let signal_app = app.clone();
    let mut events = app.subscribe_events();
    let mut shutdown = app.subscribe_shutdown();

    let connection = Connection::session().await?;
    connection.request_name(DBUS_SERVICE).await?;
    connection
        .object_server()
        .at(DBUS_PATH, OpenOneDriveBus::new(app))
        .await?;

    let bootstrap_app = signal_app.clone();
    tokio::spawn(async move {
        if let Err(error) = bootstrap_app.bootstrap().await {
            warn!("startup bootstrap failed: {error:#}");
        }
    });

    let signal_connection = connection.clone();
    tokio::spawn(async move {
        let Ok(signal_context) = zbus::SignalContext::new(&signal_connection, DBUS_PATH) else {
            return;
        };

        while let Ok(event) = events.recv().await {
            match event {
                BackendEvent::ConnectionStateChanged => {
                    if let Ok(status) = signal_app.get_status().await {
                        let _ = OpenOneDriveBus::emit_connection_state_changed(
                            &signal_context,
                            status.connection_state,
                        )
                        .await;
                    }
                }
                BackendEvent::FilesystemStateChanged => {
                    if let Ok(status) = signal_app.get_status().await {
                        let _ = OpenOneDriveBus::emit_filesystem_state_changed(
                            &signal_context,
                            status.filesystem_state,
                        )
                        .await;
                    }
                }
                BackendEvent::SyncStateChanged => {
                    if let Ok(status) = signal_app.get_status().await {
                        let _ = OpenOneDriveBus::emit_sync_state_changed(
                            &signal_context,
                            status.sync_state,
                        )
                        .await;
                    }
                }
                BackendEvent::AuthFlowStarted => {
                    let _ = OpenOneDriveBus::emit_auth_flow_started(&signal_context).await;
                }
                BackendEvent::AuthFlowCompleted => {
                    let _ = OpenOneDriveBus::emit_auth_flow_completed(&signal_context).await;
                }
                BackendEvent::ErrorRaised(message) => {
                    let _ = OpenOneDriveBus::emit_error_raised(&signal_context, &message).await;
                }
                BackendEvent::LogsUpdated => {
                    let _ = OpenOneDriveBus::emit_logs_updated(&signal_context).await;
                }
                BackendEvent::PathStatesChanged(paths) => {
                    let _ = OpenOneDriveBus::emit_path_states_changed(&signal_context, paths).await;
                }
            }
        }
    });

    info!("openonedrived ready on {DBUS_SERVICE}");
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {}
        _ = shutdown.recv() => {}
    }
    Ok(())
}

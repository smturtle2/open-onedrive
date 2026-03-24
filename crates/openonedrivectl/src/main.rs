use anyhow::Result;
use clap::{Parser, Subcommand};
use openonedrive_ipc_types::{ItemSnapshot, StatusSnapshot};
use zbus::Proxy;

const DBUS_SERVICE: &str = "io.github.smturtle2.OpenOneDrive1";
const DBUS_PATH: &str = "/io/github/smturtle2/OpenOneDrive1";
const DBUS_INTERFACE: &str = "io.github.smturtle2.OpenOneDrive1";

#[derive(Debug, Parser)]
#[command(name = "openonedrivectl")]
#[command(about = "developer CLI for the open-onedrive daemon")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Status,
    Login { client_id: String },
    SetMountPath { path: String },
    Pin { paths: Vec<String> },
    Evict { paths: Vec<String> },
    Pause,
    Resume,
    List { paths: Vec<String> },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let connection = zbus::Connection::session().await?;
    let proxy = Proxy::new(&connection, DBUS_SERVICE, DBUS_PATH, DBUS_INTERFACE).await?;

    match cli.command {
        Command::Status => {
            let status: StatusSnapshot = proxy.call("GetStatus", &()).await?;
            println!("{}", serde_json::to_string_pretty(&status)?);
        }
        Command::Login { client_id } => {
            let url: String = proxy.call("Login", &(client_id)).await?;
            println!("{url}");
        }
        Command::SetMountPath { path } => {
            proxy.call::<_, _, ()>("SetMountPath", &(path)).await?;
        }
        Command::Pin { paths } => {
            proxy.call::<_, _, ()>("Pin", &(paths)).await?;
        }
        Command::Evict { paths } => {
            proxy.call::<_, _, ()>("Evict", &(paths)).await?;
        }
        Command::Pause => {
            proxy.call::<_, _, ()>("PauseSync", &()).await?;
        }
        Command::Resume => {
            proxy.call::<_, _, ()>("ResumeSync", &()).await?;
        }
        Command::List { paths } => {
            let items: Vec<ItemSnapshot> = proxy.call("GetItems", &(paths)).await?;
            println!("{}", serde_json::to_string_pretty(&items)?);
        }
    }

    Ok(())
}


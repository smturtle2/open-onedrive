use anyhow::Result;
use clap::{Parser, Subcommand};
use openonedrive_ipc_types::{PathState, StatusSnapshot};
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
    BeginConnect,
    Disconnect,
    RepairRemote,
    SetRootPath {
        path: String,
    },
    StartFilesystem,
    StopFilesystem,
    RetryFilesystem,
    Rescan,
    PauseSync,
    ResumeSync,
    KeepLocal {
        paths: Vec<String>,
    },
    MakeOnlineOnly {
        paths: Vec<String>,
    },
    RetryTransfer {
        paths: Vec<String>,
    },
    PathStates {
        paths: Vec<String>,
    },
    Logs {
        #[arg(default_value_t = 50)]
        limit: u32,
    },
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
        Command::BeginConnect => {
            proxy.call::<_, _, ()>("BeginConnect", &()).await?;
        }
        Command::Disconnect => {
            proxy.call::<_, _, ()>("Disconnect", &()).await?;
        }
        Command::RepairRemote => {
            proxy.call::<_, _, ()>("RepairRemote", &()).await?;
        }
        Command::SetRootPath { path } => {
            proxy.call::<_, _, ()>("SetRootPath", &(path)).await?;
        }
        Command::StartFilesystem => {
            proxy.call::<_, _, ()>("StartFilesystem", &()).await?;
        }
        Command::StopFilesystem => {
            proxy.call::<_, _, ()>("StopFilesystem", &()).await?;
        }
        Command::RetryFilesystem => {
            proxy.call::<_, _, ()>("RetryFilesystem", &()).await?;
        }
        Command::Rescan => {
            let count: u32 = proxy.call("Rescan", &()).await?;
            println!("scanned {count} path state item(s)");
        }
        Command::PauseSync => {
            proxy.call::<_, _, ()>("PauseSync", &()).await?;
        }
        Command::ResumeSync => {
            proxy.call::<_, _, ()>("ResumeSync", &()).await?;
        }
        Command::KeepLocal { paths } => {
            let count: u32 = proxy.call("KeepLocal", &(paths)).await?;
            println!("kept {count} item(s) on this device");
        }
        Command::MakeOnlineOnly { paths } => {
            let count: u32 = proxy.call("MakeOnlineOnly", &(paths)).await?;
            println!("returned {count} item(s) to online-only mode");
        }
        Command::RetryTransfer { paths } => {
            let count: u32 = proxy.call("RetryTransfer", &(paths)).await?;
            println!("retried {count} transfer item(s)");
        }
        Command::PathStates { paths } => {
            let states: Vec<PathState> = proxy.call("GetPathStates", &(paths)).await?;
            println!("{}", serde_json::to_string_pretty(&states)?);
        }
        Command::Logs { limit } => {
            let lines: Vec<String> = proxy.call("GetRecentLogLines", &(limit)).await?;
            for line in lines {
                println!("{line}");
            }
        }
    }

    Ok(())
}

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use openonedrive_rclone_worker::{WorkerRequest, run_request};
use std::ffi::OsString;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process;
use std::time::Duration;

#[derive(Debug, Parser)]
#[command(name = "openonedrive-rclone-worker")]
#[command(about = "internal worker that runs isolated rclone commands")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Run {
        #[arg(long)]
        binary: PathBuf,
        #[arg(long)]
        timeout_ms: Option<u64>,
        #[arg(long)]
        stream_output: bool,
        #[arg(trailing_var_arg = true, allow_hyphen_values = true, required = true)]
        args: Vec<OsString>,
    },
}

#[tokio::main]
async fn main() {
    if let Err(error) = real_main().await {
        let _ = writeln!(io::stderr(), "{error:#}");
        process::exit(1);
    }
}

async fn real_main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Run {
            binary,
            timeout_ms,
            stream_output,
            args,
        } => {
            let result = run_request(WorkerRequest {
                binary,
                args,
                timeout: timeout_ms.map(Duration::from_millis),
                stream_output,
            })
            .await?;
            if !result.stdout.is_empty() {
                io::stdout()
                    .write_all(&result.stdout)
                    .context("failed to write worker stdout")?;
            }
            if !result.stderr.is_empty() {
                io::stderr()
                    .write_all(&result.stderr)
                    .context("failed to write worker stderr")?;
            }
            process::exit(result.status.code().unwrap_or(1));
        }
    }
}

use anyhow::{Context, Result, bail};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Stdio};
use std::time::Duration;
use tokio::process::Command;

#[derive(Debug, Clone)]
pub struct WorkerRequest {
    pub binary: PathBuf,
    pub args: Vec<OsString>,
    pub timeout: Option<Duration>,
    pub stream_output: bool,
}

#[derive(Debug)]
pub struct WorkerResult {
    pub status: ExitStatus,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

pub async fn run_request(request: WorkerRequest) -> Result<WorkerResult> {
    let WorkerRequest {
        binary,
        args,
        timeout,
        stream_output,
    } = request;

    if stream_output {
        let mut child = Command::new(&binary)
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
            .with_context(|| format!("failed to spawn {}", binary.display()))?;
        let status = wait_with_optional_timeout(child.wait(), timeout)
            .await
            .with_context(|| format!("worker command failed for {}", binary.display()))?;
        return Ok(WorkerResult {
            status,
            stdout: Vec::new(),
            stderr: Vec::new(),
        });
    }

    let child = Command::new(&binary)
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("failed to spawn {}", binary.display()))?;
    let output = wait_with_optional_timeout(child.wait_with_output(), timeout)
        .await
        .with_context(|| format!("worker command failed for {}", binary.display()))?;
    Ok(WorkerResult {
        status: output.status,
        stdout: output.stdout,
        stderr: output.stderr,
    })
}

async fn wait_with_optional_timeout<T>(
    future: impl std::future::Future<Output = std::io::Result<T>>,
    timeout: Option<Duration>,
) -> Result<T> {
    match timeout {
        Some(timeout) => match tokio::time::timeout(timeout, future).await {
            Ok(result) => result.context("command execution failed"),
            Err(_) => bail!("command timed out after {} ms", timeout.as_millis()),
        },
        None => future.await.context("command execution failed"),
    }
}

pub fn build_cli_args(
    binary: &Path,
    args: Vec<OsString>,
    timeout: Option<Duration>,
    stream_output: bool,
) -> Vec<OsString> {
    let mut cli_args = vec![
        OsString::from("run"),
        OsString::from("--binary"),
        binary.as_os_str().to_os_string(),
    ];
    if let Some(timeout) = timeout {
        cli_args.push(OsString::from("--timeout-ms"));
        cli_args.push(OsString::from(timeout.as_millis().to_string()));
    }
    if stream_output {
        cli_args.push(OsString::from("--stream-output"));
    }
    cli_args.push(OsString::from("--"));
    cli_args.extend(args);
    cli_args
}

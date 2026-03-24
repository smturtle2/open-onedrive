use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use std::env;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Debug, Parser)]
#[command(name = "xtask")]
struct Cli {
    #[command(subcommand)]
    command: Task,
}

#[derive(Debug, Subcommand)]
enum Task {
    Bootstrap,
    Check,
    Test,
    BuildUi,
    BuildIntegrations,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Task::Bootstrap => bootstrap(),
        Task::Check => {
            cargo(&["check", "--workspace"])?;
            Ok(())
        }
        Task::Test => {
            cargo(&["test", "--workspace"])?;
            Ok(())
        }
        Task::BuildUi => cmake_build("ui", "build/ui"),
        Task::BuildIntegrations => cmake_build("integrations", "build/integrations"),
    }
}

fn bootstrap() -> Result<()> {
    ensure_binary("cargo")?;
    ensure_any_binary(&["cmake", "qt-cmake", "/usr/lib/qt6/bin/qt-cmake"])?;
    ensure_any_binary(&["ninja", "make"])?;
    for binary in ["pkg-config", "qml"] {
        ensure_binary(binary)?;
    }
    println!("bootstrap checks passed");
    Ok(())
}

fn cargo(args: &[&str]) -> Result<()> {
    let status = Command::new("cargo")
        .args(args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .with_context(|| format!("failed to execute cargo {}", args.join(" ")))?;
    if !status.success() {
        bail!("cargo {} failed", args.join(" "));
    }
    Ok(())
}

fn cmake_build(source_dir: &str, build_dir: &str) -> Result<()> {
    let cmake = cmake_binary();
    let home = env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    let nix_include = home.join(".nix-profile/include");
    let nix_libx11 = home.join(".nix-profile/lib/libX11.so");
    let mut configure = Command::new(cmake);
    configure
        .args(["-S", source_dir, "-B", build_dir])
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    if has_binary("ninja") {
        configure.args(["-G", "Ninja"]);
    }
    if nix_include.exists() {
        configure.arg(format!(
            "-DX11_X11_INCLUDE_PATH={}",
            nix_include.display()
        ));
    }
    if nix_libx11.exists() {
        configure.arg(format!("-DX11_X11_LIB={}", nix_libx11.display()));
    }
    apply_cmake_env(&mut configure)?;
    run_command(configure, "cmake configure")?;

    let mut build = Command::new(cmake);
    build
        .args(["--build", build_dir])
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    apply_cmake_env(&mut build)?;
    run_command(build, "cmake build")?;
    Ok(())
}

fn apply_cmake_env(command: &mut Command) -> Result<()> {
    let prefix_path = cmake_prefix_path()?;
    let home = env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    let nix_include = home.join(".nix-profile/include");
    let nix_lib = home.join(".nix-profile/lib");

    command.env("CMAKE_PREFIX_PATH", prefix_path);
    if nix_include.exists() {
        command.env("CMAKE_INCLUDE_PATH", nix_include.as_os_str());
        command.env("X11_X11_INCLUDE_PATH", nix_include.as_os_str());
    }
    if nix_lib.exists() {
        command.env("CMAKE_LIBRARY_PATH", nix_lib.as_os_str());
        command.env("X11_X11_LIB", nix_lib.join("libX11.so").as_os_str());
    }
    if Path::new("/usr/lib/qt6/bin/qtpaths").exists() {
        command.env("QT_QTPATHS_EXECUTABLE", "/usr/lib/qt6/bin/qtpaths");
    }

    if Path::new("/usr/include/GL/gl.h").exists() {
        command.env("OPENGL_INCLUDE_DIR", "/usr/include");
    }
    if Path::new("/usr/lib/libOpenGL.so").exists() {
        command.env("OPENGL_opengl_LIBRARY", "/usr/lib/libOpenGL.so");
    }
    if Path::new("/usr/lib/libGLX.so").exists() {
        command.env("OPENGL_glx_LIBRARY", "/usr/lib/libGLX.so");
    }

    Ok(())
}

fn cmake_prefix_path() -> Result<OsString> {
    let home = env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    let mut prefixes = vec![
        home.join(".nix-profile").into_os_string(),
        PathBuf::from("/nix/var/nix/profiles/default").into_os_string(),
    ];
    if let Some(existing) = env::var_os("CMAKE_PREFIX_PATH") {
        prefixes.push(existing);
    }
    env::join_paths(prefixes).context("joining CMAKE_PREFIX_PATH")
}

fn run_command(mut command: Command, label: &str) -> Result<()> {
    let status = command
        .status()
        .with_context(|| format!("failed to execute {label}"))?;
    if !status.success() {
        bail!("{label} failed");
    }
    Ok(())
}

fn ensure_binary(binary: &str) -> Result<()> {
    if !has_binary(binary) {
        bail!("missing required tool: {binary}");
    }
    Ok(())
}

fn ensure_any_binary(binaries: &[&str]) -> Result<()> {
    if binaries.iter().any(|binary| has_binary(binary)) {
        return Ok(());
    }
    bail!("missing required tools: one of {}", binaries.join(", "))
}

fn has_binary(binary: &str) -> bool {
    let status = Command::new("sh")
        .arg("-lc")
        .arg(format!("command -v {binary} >/dev/null"))
        .status();
    matches!(status, Ok(code) if code.success())
}

fn cmake_binary() -> &'static str {
    if has_binary("qt-cmake") {
        "qt-cmake"
    } else if has_binary("/usr/lib/qt6/bin/qt-cmake") {
        "/usr/lib/qt6/bin/qt-cmake"
    } else if has_binary("cmake") {
        "cmake"
    } else {
        "/usr/bin/cmake"
    }
}

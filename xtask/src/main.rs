use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use std::env;
use std::ffi::OsString;
use std::fs;
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
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
    Install,
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
        Task::Install => install(),
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

fn install() -> Result<()> {
    bootstrap()?;
    cargo(&["build", "--workspace"])?;
    cmake_build("ui", "build/ui")?;
    cmake_build("integrations", "build/integrations")?;

    let home = env::var_os("HOME")
        .map(PathBuf::from)
        .context("HOME is not set")?;
    let prefix = home.join(".local");
    let bin_dir = prefix.join("bin");
    let libexec_dir = prefix.join("lib").join("open-onedrive");
    let app_dir = prefix.join("share").join("applications");
    let icon_dir = prefix
        .join("share")
        .join("icons")
        .join("hicolor")
        .join("scalable")
        .join("apps");
    let service_dir = home.join(".config").join("systemd").join("user");
    let plugin_root = prefix.join("lib").join("qt6").join("plugins").join("kf6");
    let action_plugin_dir = plugin_root.join("kfileitemaction");
    let overlay_plugin_dir = plugin_root.join("overlayicon");

    for dir in [
        &bin_dir,
        &libexec_dir,
        &app_dir,
        &icon_dir,
        &service_dir,
        &action_plugin_dir,
        &overlay_plugin_dir,
    ] {
        fs::create_dir_all(dir)
            .with_context(|| format!("unable to create {}", dir.display()))?;
    }

    install_file("target/debug/openonedrived", &bin_dir.join("openonedrived"), true)?;
    install_file(
        "target/debug/openonedrivectl",
        &bin_dir.join("openonedrivectl"),
        true,
    )?;
    install_file(
        "build/ui/open-onedrive-ui",
        &libexec_dir.join("open-onedrive-ui"),
        true,
    )?;
    install_file(
        "build/integrations/plugins/kf6/kfileitemaction/libopen_onedrive_fileitemaction.so",
        &action_plugin_dir.join("libopen_onedrive_fileitemaction.so"),
        false,
    )?;
    install_file(
        "build/integrations/plugins/kf6/overlayicon/libopen_onedrive_overlayicon.so",
        &overlay_plugin_dir.join("libopen_onedrive_overlayicon.so"),
        false,
    )?;
    install_file(
        "assets/open-onedrive.svg",
        &icon_dir.join("io.github.smturtle2.OpenOneDrive.svg"),
        false,
    )?;

    write_text_file(
        &bin_dir.join("open-onedrive"),
        &render_template(
            "packaging/open-onedrive-launcher.in",
            &[
                ("@INSTALL_BIN_DIR@", bin_dir.to_string_lossy().as_ref()),
                ("@INSTALL_LIBEXEC_DIR@", libexec_dir.to_string_lossy().as_ref()),
                ("@SERVICE_NAME@", "openonedrived.service"),
            ],
        )?,
        true,
    )?;

    write_text_file(
        &app_dir.join("io.github.smturtle2.OpenOneDrive.desktop"),
        &render_template("packaging/open-onedrive.desktop.in", &[])?,
        false,
    )?;

    write_text_file(
        &service_dir.join("openonedrived.service"),
        &render_template(
            "packaging/openonedrived.service.in",
            &[("@INSTALL_BIN_DIR@", bin_dir.to_string_lossy().as_ref())],
        )?,
        false,
    )?;

    let mut stop_service = Command::new("systemctl");
    stop_service.args(["--user", "stop", "openonedrived.service"]);
    run_optional(stop_service, "systemctl --user stop openonedrived.service");

    stop_repo_daemon_if_present()?;

    let mut daemon_reload = Command::new("systemctl");
    daemon_reload.args(["--user", "daemon-reload"]);
    run_optional(daemon_reload, "systemctl --user daemon-reload");

    let mut enable_service = Command::new("systemctl");
    enable_service.args(["--user", "enable", "--now", "openonedrived.service"]);
    run_optional(
        enable_service,
        "systemctl --user enable --now openonedrived.service",
    );

    let mut update_desktop_database = Command::new("update-desktop-database");
    update_desktop_database.arg(&app_dir);
    run_optional(update_desktop_database, "update-desktop-database");

    println!("Installed open-onedrive into {}", prefix.display());
    println!("Launch from your app menu or run: {}", bin_dir.join("open-onedrive").display());
    println!("Daemon service: systemctl --user status openonedrived.service");
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

fn install_file(src: &str, dst: &Path, executable: bool) -> Result<()> {
    let src_path = Path::new(src);
    if !src_path.exists() {
        bail!("missing build artifact: {}", src_path.display());
    }
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("unable to create {}", parent.display()))?;
    }
    fs::copy(src_path, dst)
        .with_context(|| format!("unable to copy {} to {}", src_path.display(), dst.display()))?;
    set_mode(dst, executable)?;
    Ok(())
}

fn write_text_file(path: &Path, content: &str, executable: bool) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("unable to create {}", parent.display()))?;
    }
    let mut file = fs::File::create(path)
        .with_context(|| format!("unable to create {}", path.display()))?;
    file.write_all(content.as_bytes())
        .with_context(|| format!("unable to write {}", path.display()))?;
    set_mode(path, executable)?;
    Ok(())
}

fn render_template(template_path: &str, replacements: &[(&str, &str)]) -> Result<String> {
    let mut content = fs::read_to_string(template_path)
        .with_context(|| format!("unable to read {}", template_path))?;
    for (needle, value) in replacements {
        content = content.replace(needle, value);
    }
    Ok(content)
}

fn set_mode(path: &Path, executable: bool) -> Result<()> {
    #[cfg(unix)]
    {
        let mode = if executable { 0o755 } else { 0o644 };
        let permissions = fs::Permissions::from_mode(mode);
        fs::set_permissions(path, permissions)
            .with_context(|| format!("unable to chmod {}", path.display()))?;
    }
    #[cfg(not(unix))]
    let _ = executable;
    Ok(())
}

fn stop_repo_daemon_if_present() -> Result<()> {
    let pid_file = Path::new(".cache").join("openonedrived.pid");
    if !pid_file.exists() {
        return Ok(());
    }

    let raw_pid = fs::read_to_string(&pid_file)
        .with_context(|| format!("unable to read {}", pid_file.display()))?;
    let pid = raw_pid.trim();
    if pid.is_empty() {
        return Ok(());
    }

    let mut kill = Command::new("kill");
    kill.arg(pid);
    run_optional(kill, &format!("kill {pid}"));
    let _ = fs::remove_file(pid_file);
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

fn run_optional(mut command: Command, label: &str) {
    match command.status() {
        Ok(status) if status.success() => {}
        Ok(status) => eprintln!("{label} skipped with exit code {status}"),
        Err(error) => eprintln!("{label} skipped: {error}"),
    }
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

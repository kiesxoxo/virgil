// Virgil.exe is a desktop app: no console window in release builds.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod api;
mod app;
mod config;
mod ctl;
mod desktop;
mod signal;
mod sync;
mod tags;
mod web;
#[cfg(windows)]
mod window;
mod ws;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Context;
use clap::{Parser, Subcommand};
use tokio::net::TcpListener;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;
use virgil_library::Library;

use crate::app::App;
use crate::config::Config;
use crate::desktop::Desktop;

#[derive(Parser)]
#[command(version, about = "Bit-perfect music player: web UI and API in front of MPD")]
struct Cli {
    /// Config file (TOML). Without one, the defaults expect MPD on 127.0.0.1:6600
    /// (on Windows, no config means desktop mode).
    #[arg(short, long, env = "VIRGIL_CONFIG")]
    config: Option<PathBuf>,
    /// Started by a desktop-mode restart: wait for the old instance to let go.
    #[arg(long, hide = true)]
    restarted: bool,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Tell the running daemon an external source takes or frees the DAC
    /// (for shairport-sync's session hooks).
    Ctl {
        action: ctl::Action,
        /// Source name from the config's `[sources]`.
        source: String,
    },
    /// Desktop mode (the default on Windows): run MPD too, and open the UI in a window.
    Desktop,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let desktop = matches!(cli.command, Some(Command::Desktop))
        || (cfg!(windows) && cli.config.is_none() && cli.command.is_none());
    if desktop {
        return run_desktop(cli.restarted);
    }
    let config = Config::load(cli.config.as_deref())?;
    if let Some(Command::Ctl { action, source }) = cli.command {
        return ctl::run(&config, action, &source);
    }
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .init();
    tokio::runtime::Builder::new_multi_thread().enable_all().build()?.block_on(async {
        let listener = TcpListener::bind(config.listen).await.with_context(|| format!("binding {}", config.listen))?;
        serve(config, listener, None).await
    })
}

/// Virgil.exe: logs go to a file (there's no console), and a failure to start opens it.
fn run_desktop(restarted: bool) -> anyhow::Result<()> {
    let base = desktop::base_dir();
    std::fs::create_dir_all(&base).with_context(|| format!("creating {}", base.display()))?;
    let log = base.join("virgil.log");
    // Appended to, not recreated: a second launch (which only brings the
    // running Virgil to the front) mustn't wipe the running one's log.
    if std::fs::metadata(&log).is_ok_and(|m| m.len() > 4 << 20) {
        let _ = std::fs::rename(&log, base.join("virgil.old.log"));
    }
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log)
        .with_context(|| format!("opening {}", log.display()))?;
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with_writer(std::sync::Mutex::new(file))
        .with_ansi(false)
        .init();
    let result = desktop_start(base, restarted, &log);
    if let Err(e) = &result {
        error!("{e:#}");
        desktop::open(&log.to_string_lossy());
    }
    result
}

/// Claims Virgil's port, or hands over to the Virgil that already has it, then
/// runs Virgil with its window (Windows) or opens the UI in a browser.
fn desktop_start(base: PathBuf, restarted: bool, #[allow(unused)] log: &std::path::Path) -> anyhow::Result<()> {
    let (desktop, config) = Desktop::prepare(base)?;
    // One Virgil at a time: if the port is taken, Virgil is already running; show it.
    // After a restart, give the old instance a moment to let go of the port.
    let deadline = Instant::now() + if restarted { Duration::from_secs(15) } else { Duration::ZERO };
    let listener = loop {
        match std::net::TcpListener::bind(config.listen) {
            Ok(listener) => break listener,
            Err(_) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(200)),
            Err(_) if !restarted => {
                info!("Virgil is already running; bringing it to the front");
                if !desktop::show_running(config.listen.port()) {
                    desktop::open_window(&desktop.url());
                }
                return Ok(());
            }
            Err(e) => return Err(e).with_context(|| format!("binding {}", config.listen)),
        }
    };
    listener.set_nonblocking(true)?;
    let desktop = Arc::new(desktop);
    let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build()?;
    let main = desktop_main(desktop.clone(), config, listener);
    #[cfg(windows)]
    window::run(runtime, desktop, main, log);
    #[cfg(not(windows))]
    {
        // A restarted Virgil's browser tab reconnects by itself.
        if !restarted {
            desktop::open_window(&desktop.url());
        }
        runtime.block_on(main)
    }
}

async fn desktop_main(desktop: Arc<Desktop>, config: Config, listener: std::net::TcpListener) -> anyhow::Result<()> {
    let listener = TcpListener::from_std(listener)?;
    desktop.start_mpd().await?;
    tokio::spawn({
        let desktop = desktop.clone();
        async move { desktop.supervise().await }
    });
    serve(config, listener, Some(desktop)).await
}

async fn serve(config: Config, listener: TcpListener, desktop: Option<Arc<Desktop>>) -> anyhow::Result<()> {
    std::fs::create_dir_all(&config.data_dir).with_context(|| format!("creating {}", config.data_dir.display()))?;
    let library = Library::open(&config.data_dir, &config.mpd, config.music_dir.clone())?;
    let listen = config.listen;
    let app = App::new(config, library, desktop);

    tokio::spawn(sync::run_mpd(app.clone()));
    tokio::spawn(sync::run_library(app.clone()));
    tokio::spawn(signal::run(app.clone()));

    info!("virgild listening on http://{listen}");
    // Open WebSockets would stall a graceful shutdown, and there is nothing to
    // flush (SQLite is in WAL mode), so just stop on a signal.
    tokio::select! {
        served = axum::serve(listener, api::router(app)) => served?,
        () = shutdown_signal() => info!("shutting down"),
    }
    Ok(())
}

#[cfg(unix)]
async fn shutdown_signal() {
    use tokio::signal::unix::{SignalKind, signal};
    let mut term = signal(SignalKind::terminate()).expect("installing SIGTERM handler");
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {}
        _ = term.recv() => {}
    }
}

#[cfg(not(unix))]
async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}

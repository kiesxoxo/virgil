// hifi.exe is a desktop app: no console window in release builds.
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
mod ws;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Context;
use clap::{Parser, Subcommand};
use hifi_library::Library;
use tokio::net::TcpListener;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

use crate::app::App;
use crate::config::Config;
use crate::desktop::Desktop;

#[derive(Parser)]
#[command(version, about = "Bit-perfect music player: web UI and API in front of MPD")]
struct Cli {
    /// Config file (TOML). Without one, the defaults expect MPD on 127.0.0.1:6600
    /// (on Windows, no config means desktop mode).
    #[arg(short, long, env = "HIFI_CONFIG")]
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

/// hifi.exe: logs go to a file (there's no console), and a failure to start opens it.
fn run_desktop(restarted: bool) -> anyhow::Result<()> {
    let base = desktop::base_dir();
    std::fs::create_dir_all(&base).with_context(|| format!("creating {}", base.display()))?;
    let log = base.join("hifi.log");
    let file = std::fs::File::create(&log).with_context(|| format!("creating {}", log.display()))?;
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with_writer(std::sync::Mutex::new(file))
        .with_ansi(false)
        .init();
    let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build()?;
    let result = runtime.block_on(desktop_main(base, restarted));
    if let Err(e) = &result {
        error!("{e:#}");
        desktop::open(&log.to_string_lossy());
    }
    result
}

async fn desktop_main(base: PathBuf, restarted: bool) -> anyhow::Result<()> {
    let (desktop, config) = Desktop::prepare(base)?;
    let url = desktop.url();
    // One hifi at a time: if the port is taken, hifi is already running; show it.
    // After a restart, give the old instance a moment to let go of the port.
    let deadline = Instant::now() + if restarted { Duration::from_secs(15) } else { Duration::ZERO };
    let listener = loop {
        match TcpListener::bind(config.listen).await {
            Ok(listener) => break listener,
            Err(_) if Instant::now() < deadline => tokio::time::sleep(Duration::from_millis(200)).await,
            Err(_) if !restarted => {
                info!("hifi is already running; opening its window");
                desktop::open_window(&url);
                return Ok(());
            }
            Err(e) => return Err(e).with_context(|| format!("binding {}", config.listen)),
        }
    };
    let desktop = Arc::new(desktop);
    desktop.start_mpd().await?;
    tokio::spawn({
        let desktop = desktop.clone();
        async move { desktop.supervise().await }
    });
    // A restarted hifi's window reconnects by itself.
    if !restarted {
        desktop::open_window(&url);
    }
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

    info!("hifid listening on http://{listen}");
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

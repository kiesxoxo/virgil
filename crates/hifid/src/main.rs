mod api;
mod app;
mod config;
mod ctl;
mod signal;
mod sync;
mod web;
mod ws;

use std::path::PathBuf;

use anyhow::Context;
use clap::{Parser, Subcommand};
use hifi_library::Library;
use tracing::info;
use tracing_subscriber::EnvFilter;

use crate::app::App;
use crate::config::Config;

#[derive(Parser)]
#[command(version, about = "Headless bit-perfect music player: web UI and API in front of MPD")]
struct Cli {
    /// Config file (TOML). Without one, the defaults expect MPD on 127.0.0.1:6600.
    #[arg(short, long, env = "HIFI_CONFIG")]
    config: Option<PathBuf>,
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
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let config = Config::load(cli.config.as_deref())?;
    if let Some(Command::Ctl { action, source }) = cli.command {
        return ctl::run(&config, action, &source);
    }
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .init();
    tokio::runtime::Builder::new_multi_thread().enable_all().build()?.block_on(serve(config))
}

async fn serve(config: Config) -> anyhow::Result<()> {
    std::fs::create_dir_all(&config.data_dir).with_context(|| format!("creating {}", config.data_dir.display()))?;
    let library = Library::open(&config.data_dir, &config.mpd)?;
    let listen = config.listen;
    let app = App::new(config, library);

    tokio::spawn(sync::run_mpd(app.clone()));
    tokio::spawn(sync::run_library(app.clone()));
    tokio::spawn(signal::run(app.clone()));

    let listener = tokio::net::TcpListener::bind(listen).await.with_context(|| format!("binding {listen}"))?;
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

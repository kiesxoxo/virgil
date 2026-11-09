mod api;
mod app;
mod config;
mod ctl;
mod homekit;
mod signal;
mod sync;
mod web;
mod ws;

use std::path::PathBuf;

use anyhow::Context;
use clap::{Parser, Subcommand};
use tokio::net::TcpListener;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;
use virgil_library::Library;

use crate::app::App;
use crate::config::Config;

#[derive(Parser)]
#[command(version, about = "Bit-perfect music player: web UI and API in front of MPD")]
struct Cli {
    /// Config file (TOML). Without one, the defaults expect MPD on 127.0.0.1:6600.
    #[arg(short, long, env = "VIRGIL_CONFIG")]
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
    tokio::runtime::Builder::new_multi_thread().enable_all().build()?.block_on(async {
        let listener = TcpListener::bind(config.listen).await.with_context(|| format!("binding {}", config.listen))?;
        serve(config, listener).await
    })
}

async fn serve(config: Config, listener: TcpListener) -> anyhow::Result<()> {
    if config.old_music_dir.is_some() {
        warn!("music_dir in the config is no longer used (Virgil only reads the music, through MPD); it can go");
    }
    std::fs::create_dir_all(&config.data_dir).with_context(|| format!("creating {}", config.data_dir.display()))?;
    let library = Library::open(&config.data_dir, &config.mpd)?;
    let radio = virgil_radio::Nts::new()?;
    let listen = config.listen;
    let (switch, writes) = homekit::start(&config).await.unzip();
    let app = App::new(config, library, radio, switch);

    tokio::spawn(sync::run_mpd(app.clone()));
    tokio::spawn(sync::run_library(app.clone()));
    tokio::spawn(signal::run(app.clone()));
    if let Some(writes) = writes {
        tokio::spawn(homekit::run(app.clone(), writes));
    }

    info!("virgild listening on http://{listen}");
    // Open WebSockets would stall a graceful shutdown, and there is nothing to
    // flush (SQLite is in WAL mode), so just stop on a signal.
    tokio::select! {
        served = axum::serve(listener, api::router(app)) => served?,
        () = shutdown_signal() => info!("shutting down"),
    }
    Ok(())
}

async fn shutdown_signal() {
    use tokio::signal::unix::{SignalKind, signal};
    let mut term = signal(SignalKind::terminate()).expect("installing SIGTERM handler");
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {}
        _ = term.recv() => {}
    }
}

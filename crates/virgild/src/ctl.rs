//! `virgild ctl acquire|release <source>`: a dependency-free HTTP client for
//! shairport-sync's session hooks.

use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::time::Duration;

use anyhow::{Context, bail};
use clap::ValueEnum;

use crate::config::Config;

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum Action {
    /// The source is about to open the DAC; returns once it is free.
    Acquire,
    /// The source has closed the DAC.
    Release,
}

pub fn run(config: &Config, action: Action, source: &str) -> anyhow::Result<()> {
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, config.listen.port()));
    let action = match action {
        Action::Acquire => "acquire",
        Action::Release => "release",
    };
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_secs(2))
        .with_context(|| format!("connecting to virgild at {addr}"))?;
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    write!(
        stream,
        "POST /api/sources/{source}/{action} HTTP/1.1\r\nHost: {addr}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
    )?;
    let mut response = String::new();
    stream.read_to_string(&mut response)?;
    let status_line = response.lines().next().unwrap_or_default();
    if status_line.split(' ').nth(1).is_some_and(|code| code.starts_with('2')) {
        Ok(())
    } else {
        bail!("virgild answered {status_line:?}")
    }
}

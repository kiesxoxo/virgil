//! MPD as hifi's playback engine.
//!
//! MPD owns decoding, gapless playback, DoP and the ALSA output; this crate
//! drives it over its text protocol. It is the seam where a native engine
//! (symphonia + alsa) could be swapped in later.

mod client;
mod engine;
mod proto;

pub use client::{Event, Mpd};
pub use engine::{Entry, MpdEngine, Song, parse_entries, parse_status};
pub use proto::{Error, Response, Result, quote};

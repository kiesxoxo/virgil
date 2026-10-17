//! Signal path: compares what the decoder produces with what ALSA reports the
//! DAC is actually receiving, so bit-perfect playback is verified, not assumed.

use std::path::Path;
use std::time::Duration;

use hifi_core::{AudioFormat, HwParams, LOCAL_SOURCE, PlayerState, SampleFormat, SignalPath, SourceState, Verdict};
use tokio::time::MissedTickBehavior;

use crate::app::App;

#[derive(Debug, Clone, PartialEq)]
pub enum Hw {
    /// No such card or substream (dev box, wrong `alsa_card`).
    Missing,
    Closed,
    Open(HwParams),
}

pub fn parse_hw_params(text: &str) -> Hw {
    let (mut format, mut rate, mut channels) = (None, None, None);
    for line in text.lines() {
        let Some((key, value)) = line.split_once(':') else { continue };
        let value = value.trim();
        match key.trim() {
            "format" => format = Some(value.to_string()),
            // "rate: 96000 (96000/1)"
            "rate" => rate = value.split_whitespace().next().and_then(|r| r.parse().ok()),
            "channels" => channels = value.parse().ok(),
            _ => {}
        }
    }
    match (format, rate, channels) {
        (Some(format), Some(rate), Some(channels)) => Hw::Open(HwParams { format, rate, channels }),
        _ => Hw::Closed,
    }
}

pub async fn read_hw(path: &Path) -> Hw {
    match tokio::fs::read_to_string(path).await {
        Ok(text) => parse_hw_params(&text),
        Err(_) => Hw::Missing,
    }
}

/// The PCM's name from `pcm0p/info` (what `aplay -l` shows), without the
/// codec part: "HiFiBerry Digi+ Pro HiFi wm8804-spdif-0" → "HiFiBerry Digi+ Pro".
pub fn parse_device_name(info: &str) -> Option<String> {
    let name = info.lines().find_map(|line| line.strip_prefix("name:"))?.trim();
    // ASoC names PCMs "<link name> <codec DAI>"; links on the Pi end in " HiFi".
    let name = name.split_once(" HiFi ").map_or(name, |(device, _)| device);
    let name = name.strip_suffix(" HiFi").unwrap_or(name).trim();
    (!name.is_empty()).then(|| name.to_string())
}

/// Significant bits of an ALSA sample format (`S24_LE` and `S24_3LE` are both 24).
fn alsa_bits(format: &str) -> Option<u8> {
    match format.split('_').next()? {
        "S8" | "U8" => Some(8),
        "S16" | "U16" => Some(16),
        "S24" | "U24" => Some(24),
        "S32" | "U32" => Some(32),
        _ => None,
    }
}

fn classify(source: &AudioFormat, output: &HwParams) -> (Verdict, bool, Option<String>) {
    if source.format == SampleFormat::Dsd {
        // DoP packs 16 DSD bits per channel into each 24-bit PCM frame.
        return if output.rate == source.rate / 16 && output.channels == source.channels {
            (Verdict::BitPerfect, true, Some("DSD carried as DoP".into()))
        } else {
            (Verdict::Converted, false, Some("DSD converted to PCM".into()))
        };
    }
    if source.format == SampleFormat::F32 {
        return (Verdict::Converted, false, Some("floating-point source converted to integer".into()));
    }
    if output.rate != source.rate {
        return (Verdict::Resampled, false, Some(format!("{} Hz → {} Hz", source.rate, output.rate)));
    }
    if output.channels != source.channels {
        return (Verdict::Converted, false, Some(format!("{} → {} channels", source.channels, output.channels)));
    }
    match alsa_bits(&output.format) {
        Some(bits) if bits >= source.format.bits() => (Verdict::BitPerfect, false, None),
        Some(bits) => {
            (Verdict::Converted, false, Some(format!("{}-bit truncated to {bits}-bit", source.format.bits())))
        }
        None => (Verdict::Unknown, false, Some(format!("unrecognised ALSA format {}", output.format))),
    }
}

pub fn evaluate(hw: &Hw, player: &PlayerState, source: &SourceState) -> SignalPath {
    match hw {
        Hw::Missing => SignalPath {
            verdict: Verdict::Unknown,
            source: player.audio,
            note: Some("ALSA card not found".into()),
            ..SignalPath::default()
        },
        Hw::Closed => SignalPath::default(),
        Hw::Open(output) if source.active != LOCAL_SOURCE => SignalPath {
            verdict: Verdict::External,
            output: Some(output.clone()),
            note: Some(source.active.clone()),
            ..SignalPath::default()
        },
        Hw::Open(output) => {
            let Some(audio) = player.audio else {
                return SignalPath {
                    verdict: Verdict::Unknown,
                    output: Some(output.clone()),
                    note: Some("the device is open but MPD is not playing".into()),
                    ..SignalPath::default()
                };
            };
            let (verdict, dop, note) = classify(&audio, output);
            SignalPath { verdict, source: Some(audio), output: Some(output.clone()), dop, note, device: None }
        }
    }
}

/// Re-evaluates the signal path twice a second; only changes are broadcast.
pub async fn run(app: App) {
    let path = app.config.hw_params_path();
    let info_path = path.with_file_name("info");
    let mut device: Option<String> = None;
    let mut tick = tokio::time::interval(Duration::from_millis(500));
    tick.set_missed_tick_behavior(MissedTickBehavior::Skip);
    loop {
        tick.tick().await;
        let hw = read_hw(&path).await;
        // The name never changes; look it up until the card is there.
        if device.is_none() && hw != Hw::Missing {
            device = tokio::fs::read_to_string(&info_path).await.ok().as_deref().and_then(parse_device_name);
        }
        let mut signal = evaluate(&hw, &app.player(), &app.source());
        signal.device = device.clone();
        app.set_signal(signal);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open(format: &str, rate: u32) -> Hw {
        Hw::Open(HwParams { format: format.into(), rate, channels: 2 })
    }

    fn verdict(source: &str, hw: &Hw) -> SignalPath {
        let player = PlayerState { audio: AudioFormat::parse_mpd(source), ..PlayerState::default() };
        evaluate(hw, &player, &SourceState::default())
    }

    #[test]
    fn parses_proc_hw_params() {
        let text = "access: RW_INTERLEAVED\nformat: S24_LE\nsubformat: STD\nchannels: 2\n\
                    rate: 96000 (96000/1)\nperiod_size: 2048\nbuffer_size: 8192\n";
        assert_eq!(parse_hw_params(text), open("S24_LE", 96_000));
        assert_eq!(parse_hw_params("closed\n"), Hw::Closed);
    }

    #[test]
    fn names_the_device_like_aplay_without_the_codec() {
        let info = "card: 0\ndevice: 0\nsubdevice: 0\nstream: PLAYBACK\n\
                    id: HiFiBerry Digi+ Pro HiFi wm8804-spdif-0\n\
                    name: HiFiBerry Digi+ Pro HiFi wm8804-spdif-0\nsubname: subdevice #0\n";
        assert_eq!(parse_device_name(info).as_deref(), Some("HiFiBerry Digi+ Pro"));
        assert_eq!(parse_device_name("name: USB Audio\n").as_deref(), Some("USB Audio"));
        assert_eq!(parse_device_name("name: \n"), None);
        assert_eq!(parse_device_name("closed\n"), None);
    }

    #[test]
    fn classifies_signal_paths() {
        assert_eq!(verdict("96000:24:2", &open("S24_LE", 96_000)).verdict, Verdict::BitPerfect);
        // Zero-padding 16-bit into a 32-bit container loses nothing.
        assert_eq!(verdict("44100:16:2", &open("S32_LE", 44_100)).verdict, Verdict::BitPerfect);
        assert_eq!(verdict("352800:24:2", &open("S24_LE", 192_000)).verdict, Verdict::Resampled);
        assert_eq!(verdict("44100:f:2", &open("S24_LE", 44_100)).verdict, Verdict::Converted);
        assert_eq!(verdict("44100:32:2", &open("S24_LE", 44_100)).verdict, Verdict::Converted);

        let dop = verdict("dsd64:2", &open("S24_LE", 176_400));
        assert_eq!((dop.verdict, dop.dop), (Verdict::BitPerfect, true));
        assert_eq!(verdict("dsd128:2", &open("S24_LE", 192_000)).verdict, Verdict::Converted);

        assert_eq!(verdict("44100:16:2", &Hw::Closed).verdict, Verdict::Idle);
        assert_eq!(verdict("44100:16:2", &Hw::Missing).verdict, Verdict::Unknown);
    }

    #[test]
    fn external_sources_are_not_judged() {
        let source = SourceState { active: "airplay".into(), available: vec!["airplay".into()] };
        let path = evaluate(&open("S16_LE", 44_100), &PlayerState::default(), &source);
        assert_eq!(path.verdict, Verdict::External);
        assert_eq!(path.note.as_deref(), Some("airplay"));
    }
}

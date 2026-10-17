//! Desktop mode: hifi.exe on a PC. hifid runs MPD itself (the official
//! mpd.exe, embedded in release builds), with one WASAPI exclusive-mode output
//! per playback device, and opens the UI as an app window. Everything lives in
//! `%LOCALAPPDATA%\hifi`.

use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex as StdMutex, OnceLock};
use std::time::Duration;

use anyhow::{Context, bail};
use hifi_core::{DesktopSettings, HwParams, PlayState, PlayerState};
use tokio::io::AsyncBufReadExt;
use tokio::process::{Child, Command};
use tokio::sync::Mutex;
use tracing::{info, warn};

use crate::config::Config;
use crate::signal::Hw;

/// MPD listens here, away from a stock MPD's 6600.
const MPD_PORT: u16 = 6612;
const DEFAULT_PORT: u16 = 8484;

#[cfg(feature = "bundled-mpd")]
const MPD_EXE: &[u8] = include_bytes!(env!("HIFI_MPD_EXE"));

/// Don't flash a console window for child processes.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub struct Desktop {
    pub base: PathBuf,
    settings: StdMutex<DesktopSettings>,
    mpd_exe: PathBuf,
    child: Mutex<Option<Child>>,
    quitting: AtomicBool,
    /// The format WASAPI last accepted in exclusive mode; None after a failed open.
    opened: Arc<StdMutex<Option<HwParams>>>,
    /// Brings the app window to the front (set once the window exists).
    show_window: OnceLock<Box<dyn Fn() + Send + Sync>>,
}

/// A playback device as Windows names it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Device {
    pub name: String,
    pub default: bool,
}

/// `%LOCALAPPDATA%\hifi` (or `~/.local/share/hifi` when trying desktop mode elsewhere).
pub fn base_dir() -> PathBuf {
    let root = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")))
        .unwrap_or_else(|| PathBuf::from("."));
    root.join("hifi")
}

fn settings_path(base: &Path) -> PathBuf {
    base.join("settings.json")
}

fn default_music_dir() -> PathBuf {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map_or_else(|| PathBuf::from("."), |home| PathBuf::from(home).join("Music"))
}

pub fn load_settings(base: &Path) -> DesktopSettings {
    std::fs::read(settings_path(base)).ok().and_then(|bytes| serde_json::from_slice(&bytes).ok()).unwrap_or_else(|| {
        DesktopSettings { music_dir: default_music_dir().to_string_lossy().into_owned(), port: DEFAULT_PORT }
    })
}

/// MPD's config syntax: forward slashes (Windows takes them), quotes escaped.
fn mpd_string(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "/").replace('"', "\\\""))
}

/// MPD's config: the library and state under `base`, one exclusive WASAPI
/// output per device. Only the default device starts enabled; after that,
/// MPD's state file remembers which one was picked.
pub fn mpd_conf(base: &Path, music_dir: &str, devices: &[Device]) -> String {
    let dir = |name: &str| mpd_string(&base.join("mpd").join(name).to_string_lossy());
    let mut conf = format!(
        "music_directory      {music}\n\
         playlist_directory   {playlists}\n\
         db_file              {db}\n\
         state_file           {state}\n\
         sticker_file         {stickers}\n\
         # Logs go to hifid over stderr; \"verbose\" includes the format\n\
         # WASAPI accepted, which the signal path shows.\n\
         log_level            \"verbose\"\n\
         bind_to_address      \"127.0.0.1\"\n\
         port                 \"{MPD_PORT}\"\n\
         restore_paused       \"yes\"\n\
         replaygain           \"off\"\n\
         volume_normalization \"no\"\n\
         max_output_buffer_size \"65536\"\n",
        music = mpd_string(music_dir),
        playlists = dir("playlists"),
        db = dir("database"),
        state = dir("state"),
        stickers = dir("sticker.sql"),
    );
    for device in devices {
        conf.push_str(&format!(
            "\naudio_output {{\n\
             \ttype       \"wasapi\"\n\
             \tname       {name}\n\
             \tdevice     {name}\n\
             \texclusive  \"yes\"\n\
             \tmixer_type \"none\"\n\
             \tenabled    \"{enabled}\"\n\
             }}\n",
            name = mpd_string(&device.name),
            enabled = if device.default { "yes" } else { "no" },
        ));
    }
    if devices.is_empty() {
        // Nothing to play to (yet); MPD needs an output to start.
        conf.push_str("\naudio_output {\n\ttype \"null\"\n\tname \"No playback device\"\n}\n");
    }
    conf
}

/// What MPD's log (at "verbose") says about a WASAPI output.
#[derive(Debug, Clone, PartialEq)]
pub enum OutputLog {
    /// `wasapi_output: Packing data: shift8=true pack24=false`, just before an open:
    /// whether 24-bit samples go out packed in 3 bytes or in a 32-bit container.
    Packing { pack24: bool },
    /// `output: opened "Speakers (DX5 II)" (wasapi) audio_format=44100:24:2`: the
    /// format MPD hands the device after any conversion of its own. In exclusive
    /// mode Windows converts nothing, so this is what the DAC receives.
    Opened { rate: u32, sample: String, channels: u8 },
    /// `output: closed "Speakers (DX5 II)" (wasapi)`
    Closed,
}

pub fn parse_output_log(line: &str) -> Option<OutputLog> {
    if let Some((_, packing)) = line.split_once("wasapi_output: Packing data: ") {
        return Some(OutputLog::Packing { pack24: packing.contains("pack24=true") });
    }
    if line.contains("output: closed \"") && line.ends_with("(wasapi)") {
        return Some(OutputLog::Closed);
    }
    let (_, rest) = line.split_once("output: opened \"")?;
    let (_, format) = rest.split_once("(wasapi) audio_format=")?;
    let mut fields = format.trim().split(':');
    let rate = fields.next()?.parse().ok()?;
    let sample = fields.next()?.to_string();
    let channels = fields.next()?.parse().ok()?;
    Some(OutputLog::Opened { rate, sample, channels })
}

/// An opened format in ALSA's terms, which the signal path compares against.
fn hw_params(rate: u32, sample: &str, channels: u8, pack24: bool) -> HwParams {
    let format = match sample {
        "8" => "S8".to_string(),
        "f" => "FLOAT_LE".to_string(),
        "24" if pack24 => "S24_3LE".to_string(),
        // 24 bits in a 32-bit container, as ALSA's S24_LE.
        "24" => "S24_LE".to_string(),
        bits => format!("S{bits}_LE"),
    };
    HwParams { format, rate, channels }
}

/// Follows MPD's log on stderr: records the format each WASAPI open settles
/// on, and passes MPD's own messages on to hifi.log, leaving out the
/// per-command and per-file chatter "verbose" adds.
async fn read_mpd_log(stderr: tokio::process::ChildStderr, opened: Arc<StdMutex<Option<HwParams>>>) {
    let mut lines = tokio::io::BufReader::new(stderr).split(b'\n');
    let mut pack24 = false;
    while let Ok(Some(bytes)) = lines.next_segment().await {
        let line = String::from_utf8_lossy(&bytes);
        let line = line.trim_end();
        match parse_output_log(line) {
            Some(OutputLog::Packing { pack24: p }) => pack24 = p,
            Some(OutputLog::Opened { rate, sample, channels }) => {
                *opened.lock().expect("opened lock") = Some(hw_params(rate, &sample, channels, pack24));
            }
            Some(OutputLog::Closed) => *opened.lock().expect("opened lock") = None,
            None => {}
        }
        // FFmpeg's warnings continue on lines of their own ("Consider increasing…").
        let chatter =
            ["client: ", "update: ", "inotify: ", "ffmpeg/", "Consider increasing"].iter().any(|p| line.contains(p));
        if !line.is_empty() && !chatter {
            info!(target: "mpd", "{line}");
        }
    }
}

/// Active playback devices, default first.
#[cfg(windows)]
pub fn devices() -> Vec<Device> {
    use wasapi::{DeviceEnumerator, Direction};
    let list = || -> Result<Vec<Device>, Box<dyn std::error::Error>> {
        let _ = wasapi::initialize_mta();
        let enumerator = DeviceEnumerator::new()?;
        let default = enumerator.get_default_device(&Direction::Render).and_then(|d| d.get_id()).ok();
        let collection = enumerator.get_device_collection(&Direction::Render)?;
        let mut devices = Vec::new();
        for i in 0..collection.get_nbr_devices()? {
            let device = collection.get_device_at_index(i)?;
            let is_default = default.as_ref().is_some_and(|id| device.get_id().is_ok_and(|d| d == *id));
            devices.push(Device { name: device.get_friendlyname()?, default: is_default });
        }
        devices.sort_by_key(|d| !d.default);
        Ok(devices)
    };
    list().unwrap_or_else(|e| {
        warn!("listing playback devices: {e}");
        Vec::new()
    })
}

#[cfg(not(windows))]
pub fn devices() -> Vec<Device> {
    Vec::new()
}

/// Sends one command to the MPD on `MPD_PORT`, if one is there.
fn mpd_command(command: &str) -> bool {
    let addr = SocketAddr::from(([127, 0, 0, 1], MPD_PORT));
    let Ok(mut stream) = TcpStream::connect_timeout(&addr, Duration::from_millis(500)) else { return false };
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let mut greeting = String::new();
    if BufReader::new(&stream).read_line(&mut greeting).is_err() || !greeting.starts_with("OK MPD") {
        return false;
    }
    stream.write_all(format!("{command}\n").as_bytes()).is_ok()
}

fn mpd_listening() -> bool {
    TcpStream::connect_timeout(&SocketAddr::from(([127, 0, 0, 1], MPD_PORT)), Duration::from_millis(200)).is_ok()
}

/// The mpd.exe to run: the embedded one, unpacked next to its data, or one
/// placed beside hifi.exe (development builds).
fn mpd_exe(base: &Path) -> anyhow::Result<PathBuf> {
    #[cfg(feature = "bundled-mpd")]
    {
        let path = base.join("mpd").join("mpd.exe");
        if std::fs::read(&path).ok().as_deref() != Some(MPD_EXE) {
            std::fs::write(&path, MPD_EXE).with_context(|| format!("unpacking {}", path.display()))?;
        }
        Ok(path)
    }
    #[cfg(not(feature = "bundled-mpd"))]
    {
        let _ = base;
        let beside = std::env::current_exe()?.with_file_name(if cfg!(windows) { "mpd.exe" } else { "mpd" });
        if beside.is_file() {
            return Ok(beside);
        }
        bail!("mpd.exe not found next to hifi (this build doesn't embed it)")
    }
}

impl Desktop {
    /// Sets up `base` and returns the daemon's config for desktop mode.
    pub fn prepare(base: PathBuf) -> anyhow::Result<(Self, Config)> {
        for dir in [base.join("mpd").join("playlists"), base.join("data")] {
            std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
        }
        let settings = load_settings(&base);
        let config = Config {
            listen: ([127, 0, 0, 1], settings.port).into(),
            mpd: format!("127.0.0.1:{MPD_PORT}"),
            data_dir: base.join("data"),
            music_dir: Some(PathBuf::from(&settings.music_dir)),
            // No ALSA here; the signal path says it can't verify the device.
            asound_dir: base.join("no-alsa"),
            ..Config::default()
        };
        let mpd_exe = mpd_exe(&base)?;
        Ok((
            Self {
                base,
                settings: StdMutex::new(settings),
                mpd_exe,
                child: Mutex::new(None),
                quitting: AtomicBool::new(false),
                opened: Arc::default(),
                show_window: OnceLock::new(),
            },
            config,
        ))
    }

    pub fn settings(&self) -> DesktopSettings {
        self.settings.lock().expect("settings lock").clone()
    }

    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.settings().port)
    }

    /// Called by the app window (Windows only).
    #[cfg_attr(not(windows), allow(dead_code))]
    pub fn set_show_window(&self, show: Box<dyn Fn() + Send + Sync>) {
        let _ = self.show_window.set(show);
    }

    /// Brings hifi to the front: its own window, else (no window) a browser.
    pub fn show_window(&self) {
        match self.show_window.get() {
            Some(show) => show(),
            None => open_window(&self.url()),
        }
    }

    /// Saves new settings; they apply after [`Desktop::restart`].
    pub fn save_settings(&self, settings: DesktopSettings) -> anyhow::Result<()> {
        if !Path::new(&settings.music_dir).is_dir() {
            bail!("{} is not a folder", settings.music_dir);
        }
        std::fs::write(settings_path(&self.base), serde_json::to_vec_pretty(&settings)?)?;
        *self.settings.lock().expect("settings lock") = settings;
        Ok(())
    }

    /// Writes MPD's config for the devices present now and starts MPD,
    /// replacing one left over from an earlier run.
    pub async fn start_mpd(&self) -> anyhow::Result<()> {
        let devices = tokio::task::spawn_blocking(devices).await?;
        info!("playback devices: {:?}", devices.iter().map(|d| &d.name).collect::<Vec<_>>());
        let conf_path = self.base.join("mpd").join("mpd.conf");
        std::fs::write(&conf_path, mpd_conf(&self.base, &self.settings().music_dir, &devices))?;

        if mpd_command("kill") {
            info!("stopping an MPD left over from an earlier run");
            for _ in 0..50 {
                if !mpd_listening() {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
        let mut command = Command::new(&self.mpd_exe);
        command
            .arg("--stderr")
            .arg(&conf_path)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true);
        #[cfg(windows)]
        command.creation_flags(CREATE_NO_WINDOW);
        let mut child = command.spawn().with_context(|| format!("starting {}", self.mpd_exe.display()))?;
        *self.opened.lock().expect("opened lock") = None;
        if let Some(stderr) = child.stderr.take() {
            tokio::spawn(read_mpd_log(stderr, self.opened.clone()));
        }
        *self.child.lock().await = Some(child);
        for _ in 0..100 {
            if mpd_listening() {
                // No auto_update on Windows: rescan for changes on every start.
                mpd_command("update");
                return Ok(());
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        bail!("MPD didn't start; its messages are in {}", self.base.join("hifi.log").display())
    }

    /// What the device is being fed: the format WASAPI accepted, while MPD
    /// plays or is paused. In exclusive mode nothing sits between MPD and the
    /// device, so this is what the DAC receives.
    pub fn hw(&self, player: &PlayerState) -> Hw {
        match (player.state, self.opened.lock().expect("opened lock").clone()) {
            (PlayState::Stop, _) => Hw::Closed,
            (_, Some(params)) => Hw::Open(params),
            // Paused since hifi started: MPD hasn't opened the device yet.
            (PlayState::Pause, None) => Hw::Closed,
            (PlayState::Play, None) => Hw::Missing,
        }
    }

    /// Restarts MPD if it exits on its own.
    pub async fn supervise(&self) {
        loop {
            let status = {
                let mut child = self.child.lock().await;
                match child.as_mut() {
                    Some(child) => child.try_wait(),
                    None => Ok(None),
                }
            };
            match status {
                Ok(Some(status)) if !self.quitting.load(Ordering::SeqCst) => {
                    warn!("MPD exited ({status}); starting it again");
                    tokio::time::sleep(Duration::from_secs(2)).await;
                    if let Err(e) = self.start_mpd().await {
                        warn!("{e:#}");
                    }
                }
                _ => tokio::time::sleep(Duration::from_secs(1)).await,
            }
        }
    }

    /// Stops MPD the polite way (it saves the queue and position) and exits.
    pub async fn quit(&self) -> ! {
        self.quitting.store(true, Ordering::SeqCst);
        info!("quitting");
        tokio::task::spawn_blocking(|| mpd_command("kill")).await.ok();
        if let Some(mut child) = self.child.lock().await.take() {
            if tokio::time::timeout(Duration::from_secs(5), child.wait()).await.is_err() {
                let _ = child.kill().await;
            }
        }
        std::process::exit(0)
    }

    /// Starts a fresh hifi (new devices, another music folder) and quits this one.
    pub async fn restart(&self) -> ! {
        match std::env::current_exe() {
            Ok(exe) => {
                let mut command = std::process::Command::new(exe);
                command.arg("--restarted");
                #[cfg(windows)]
                {
                    use std::os::windows::process::CommandExt;
                    command.creation_flags(CREATE_NO_WINDOW);
                }
                if let Err(e) = command.spawn() {
                    warn!("restarting: {e}");
                }
            }
            Err(e) => warn!("restarting: {e}"),
        }
        self.quit().await
    }
}

/// Asks the hifi already running on `port` to bring its window to the front.
pub fn show_running(port: u16) -> bool {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let Ok(mut stream) = TcpStream::connect_timeout(&addr, Duration::from_secs(1)) else { return false };
    let _ = stream.set_read_timeout(Some(Duration::from_secs(3)));
    let request =
        "POST /api/desktop/show HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
    if stream.write_all(request.as_bytes()).is_err() {
        return false;
    }
    let mut status = String::new();
    BufReader::new(&stream).read_line(&mut status).is_ok() && status.split_whitespace().nth(1) == Some("204")
}

/// Edge or Chrome, whichever is installed: both open a URL as an app window (`--app`).
#[cfg(windows)]
fn app_browser() -> Option<PathBuf> {
    let roots = ["ProgramFiles(x86)", "ProgramFiles", "LOCALAPPDATA"].map(std::env::var_os);
    let browsers = [r"Microsoft\Edge\Application\msedge.exe", r"Google\Chrome\Application\chrome.exe"];
    browsers
        .iter()
        .flat_map(|browser| roots.iter().flatten().map(move |root| PathBuf::from(root).join(browser)))
        .find(|path| path.is_file())
}

/// Opens the UI as an app window (Edge or Chrome), else in the default browser.
/// Never waits: this runs before hifi starts serving.
pub fn open_window(url: &str) {
    #[cfg(windows)]
    {
        if let Some(browser) = app_browser()
            && std::process::Command::new(browser).arg(format!("--app={url}")).spawn().is_ok()
        {
            return;
        }
    }
    open(url);
}

/// Opens a URL or file with its default app.
pub fn open(target: &str) {
    #[cfg(windows)]
    let command = {
        use std::os::windows::process::CommandExt;
        // Unlike `cmd /c start`, this shows no console and no error dialog.
        std::process::Command::new("rundll32")
            .args(["url.dll,FileProtocolHandler", target])
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
    };
    #[cfg(not(windows))]
    let command =
        std::process::Command::new(if cfg!(target_os = "macos") { "open" } else { "xdg-open" }).arg(target).spawn();
    if let Err(e) = command {
        warn!("opening {target}: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_what_mpd_opened_the_device_with() {
        // As MPD 0.24 logs them on Windows (with its own timestamp in front).
        let packing = "2026-10-17T22:45:12 wasapi_output: Packing data: shift8=true pack24=false";
        let opened = "2026-10-17T22:45:12 output: opened \"Speakers (DX5 II)\" (wasapi) audio_format=44100:24:2";
        let closed = "2026-10-17T22:45:12 output: closed \"Speakers (DX5 II)\" (wasapi)";
        assert_eq!(parse_output_log(packing), Some(OutputLog::Packing { pack24: false }));
        assert_eq!(
            parse_output_log(opened),
            Some(OutputLog::Opened { rate: 44_100, sample: "24".into(), channels: 2 })
        );
        assert_eq!(parse_output_log(closed), Some(OutputLog::Closed));
        assert_eq!(parse_output_log("client: [3] process command \"status\""), None);

        assert_eq!(hw_params(44_100, "24", 2, false), HwParams { format: "S24_LE".into(), rate: 44_100, channels: 2 });
        assert_eq!(hw_params(96_000, "24", 2, true).format, "S24_3LE");
        assert_eq!(hw_params(44_100, "16", 2, false).format, "S16_LE");
        assert_eq!(hw_params(384_000, "f", 2, false).format, "FLOAT_LE");
    }

    #[test]
    fn writes_one_exclusive_output_per_device() {
        let devices = [
            Device { name: "Speakers (Realtek(R) Audio)".into(), default: true },
            Device { name: "Headphones \"DT 770\"".into(), default: false },
        ];
        let conf = mpd_conf(Path::new(r"C:\Users\me\AppData\Local\hifi"), r"\\pi\sn700\Music", &devices);
        assert!(conf.contains("music_directory      \"//pi/sn700/Music\""));
        assert!(conf.contains("db_file              \"C:/Users/me/AppData/Local/hifi/mpd/database\""));
        assert!(conf.contains("port                 \"6612\""));
        assert_eq!(conf.matches("type       \"wasapi\"").count(), 2);
        assert!(conf.contains("\tname       \"Speakers (Realtek(R) Audio)\"\n"));
        assert!(conf.contains("\tdevice     \"Headphones \\\"DT 770\\\"\"\n"));
        assert_eq!(conf.matches("enabled    \"yes\"").count(), 1);
        assert!(mpd_conf(Path::new("/x"), "/m", &[]).contains("type \"null\""));
    }
}

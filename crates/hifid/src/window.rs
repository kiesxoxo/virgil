//! hifi.exe's own window: the web UI in WebView2 (part of Windows), so the
//! desktop app needs no browser. Closing the window quits hifi.

use std::path::Path;
use std::sync::{Arc, Mutex};

use tao::dpi::LogicalSize;
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tao::window::{Icon, Theme, Window, WindowBuilder};
use tracing::{error, info, warn};
use wry::{WebContext, WebView, WebViewBuilder};

use crate::desktop::{self, Desktop};

const ICON: &[u8] = include_bytes!("../assets/icon-256.png");

#[derive(Debug, Clone, Copy)]
enum UserEvent {
    /// Another launch of hifi.exe asked for the window.
    Show,
}

fn icon() -> Option<Icon> {
    let image = image::load_from_memory(ICON).ok()?.into_rgba8();
    let (width, height) = image.dimensions();
    Icon::from_rgba(image.into_raw(), width, height).ok()
}

/// The UI in WebView2, painted in the app's own background colour until the
/// page draws so there's no white flash in dark mode.
fn webview(window: &Window, url: &str, context: &mut WebContext) -> wry::Result<WebView> {
    let background = match window.theme() {
        Theme::Dark => (0x0b, 0x0b, 0x0d, 0xff),
        _ => (0xff, 0xff, 0xff, 0xff),
    };
    WebViewBuilder::new_with_web_context(context)
        .with_url(url)
        .with_background_color(background)
        .with_devtools(cfg!(debug_assertions))
        .build(window)
}

fn raise(window: &Window) {
    window.set_visible(true);
    window.set_minimized(false);
    window.set_focus();
}

/// Runs the window on this (the main) thread until hifi quits; `serve` runs
/// hifi itself on `runtime`. If WebView2 can't start, the UI opens in a
/// browser instead and this thread just keeps the process alive.
pub fn run<F>(runtime: tokio::runtime::Runtime, desktop: Arc<Desktop>, serve: F, log: &Path) -> !
where
    F: Future<Output = anyhow::Result<()>> + Send + 'static,
{
    let url = desktop.url();
    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let proxy = Mutex::new(event_loop.create_proxy());
    desktop.set_show_window(Box::new(move || {
        let _ = proxy.lock().expect("proxy lock").send_event(UserEvent::Show);
    }));

    let log = log.to_path_buf();
    runtime.spawn(async move {
        if let Err(e) = serve.await {
            error!("{e:#}");
            desktop::open(&log.to_string_lossy());
            std::process::exit(1);
        }
    });

    let window = WindowBuilder::new()
        .with_title("hifi")
        .with_window_icon(icon())
        .with_inner_size(LogicalSize::new(1280.0, 840.0))
        .with_min_inner_size(LogicalSize::new(360.0, 560.0))
        .build(&event_loop);
    let mut context = WebContext::new(Some(desktop.base.join("webview")));
    let view = match &window {
        Ok(window) => webview(window, &url, &mut context).map_err(|e| e.to_string()),
        Err(e) => Err(e.to_string()),
    };
    let (window, view) = match (window, view) {
        (Ok(window), Ok(view)) => (Some(window), Some(view)),
        (_, Err(e)) => {
            warn!("no app window ({e}); opening the UI in a browser");
            desktop::open_window(&url);
            (None, None)
        }
        (Err(_), Ok(_)) => unreachable!("a webview needs a window"),
    };
    info!("app window open");

    let handle = runtime.handle().clone();
    let mut closing = false;
    event_loop.run(move |event, _, control_flow| {
        // Keep the runtime (hifi itself) and the webview alive as long as the loop.
        let _ = (&runtime, &view);
        *control_flow = ControlFlow::Wait;
        match event {
            Event::WindowEvent { event: WindowEvent::CloseRequested, .. } if !closing => {
                closing = true;
                if let Some(window) = &window {
                    window.set_visible(false);
                }
                // Stops MPD (it saves the queue and position) and exits.
                let desktop = desktop.clone();
                handle.spawn(async move { desktop.quit().await });
            }
            Event::UserEvent(UserEvent::Show) => match &window {
                Some(window) => raise(window),
                None => desktop::open_window(&url),
            },
            _ => {}
        }
    })
}

#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

mod assets;
mod state;
mod theme;
mod ui;

use std::borrow::Cow;

use gpui_kit::*;

use state::app_state::AppState;

const FONTS: [(&str, &[u8]); 3] = [
    ("Fraunces", include_bytes!("../assets/fonts/Fraunces.ttf")),
    ("Inter", include_bytes!("../assets/fonts/Inter.ttf")),
    (
        "JetBrains Mono",
        include_bytes!("../assets/fonts/JetBrainsMono.ttf"),
    ),
];

/// Registers the bundled fonts. A font that fails to load is logged and GPUI falls back to a
/// system font for that family.
fn load_fonts(cx: &mut App) {
    for (name, bytes) in FONTS {
        if let Err(e) = cx.text_system().add_fonts(vec![Cow::Borrowed(bytes)]) {
            log::warn!("cannot load font {name}: {e}");
        }
    }
}

fn bind_keys(cx: &mut App) {
    let (next, prev, settings, find) = if cfg!(target_os = "macos") {
        ("cmd-right", "cmd-left", "cmd-,", "cmd-f")
    } else {
        ("ctrl-right", "ctrl-left", "ctrl-,", "ctrl-f")
    };
    cx.bind_keys([
        KeyBinding::new("space", ui::TogglePlay, Some("Aurum")),
        KeyBinding::new(next, ui::NextTrack, Some("Aurum")),
        KeyBinding::new(prev, ui::PrevTrack, Some("Aurum")),
        KeyBinding::new(settings, ui::OpenSettings, Some("Aurum")),
        KeyBinding::new(find, ui::FocusSearch, Some("Aurum")),
        // The search input's own Escape binding ends with `cx.propagate()`, so this action
        // also runs while the input has focus.
        KeyBinding::new("escape", ui::ClearSearch, Some("Aurum")),
    ]);
}

/// Opens `aurum.log` in the data dir for appending; truncates it first when it exceeds 2 MB.
fn open_log_file() -> Option<std::fs::File> {
    let dir = state::paths::data_dir();
    std::fs::create_dir_all(&dir).ok()?;
    let path = dir.join("aurum.log");
    if std::fs::metadata(&path).is_ok_and(|m| m.len() > 2 * 1024 * 1024) {
        let _ = std::fs::remove_file(&path);
    }
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .ok()
}

fn init_logging() {
    let mut builder =
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"));
    // A release build on Windows has no console, so stderr goes nowhere.
    if cfg!(target_os = "windows")
        && let Some(file) = open_log_file()
    {
        builder.target(env_logger::Target::Pipe(Box::new(file)));
    }
    builder.init();
}

fn main() {
    init_logging();

    gpui_kit::application()
        .with_assets(assets::Assets)
        .run(|cx| {
            gpui_kit::init(cx);
            theme::apply(cx);
            load_fonts(cx);
            bind_keys(cx);
            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();

            let bounds = Bounds::centered(None, size(px(1200.), px(760.)), cx);
            // Transparent title bar: macOS keeps the traffic lights over our own background,
            // Windows gets no white system bar. `window_options` makes the app own dragging
            // (see `ui::shell::drag_strip`).
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("Aurum".into()),
                    appears_transparent: true,
                    traffic_light_position: Some(point(px(16.), px(12.))),
                }),
                window_min_size: Some(size(px(900.), px(560.))),
                ..component::TitleBar::window_options()
            };
            let opened = gpui_kit::open_window(options, cx, |window, cx| {
                let state = cx.new(|cx| AppState::new(window, cx));
                let focus = state.read(cx).focus.clone();
                window.focus(&focus, cx);
                state
            });
            if let Err(e) = opened {
                log::error!("cannot open window: {e}");
                cx.quit();
                return;
            }
            cx.activate(true);
        });
}

//! Tray icon — the only persistent UI element.
//! States: IDLE (gray icon), WATCHING (red icon). OFF = app quit, no icon.

use crate::state::{AppState, WatchState};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager};

const TRAY_ID: &str = "ghlg-tray";

const ICON_IDLE: &[u8] = include_bytes!("../icons/tray-idle.png");
const ICON_WATCHING: &[u8] = include_bytes!("../icons/tray-watching.png");

pub fn init(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open-review", "Open Review Window", true, None::<&str>)?;
    let start = MenuItem::with_id(app, "start-watching", "Start Watching", true, None::<&str>)?;
    let stop = MenuItem::with_id(app, "stop-watching", "Stop Watching", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Ghostlog", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &start, &stop, &quit])?;

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(tauri::image::Image::from_bytes(ICON_IDLE)?)
        .tooltip("Ghostlog — idle")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open-review" => show_review_window(app),
            "start-watching" => {
                if let Err(e) = crate::watcher::start(app) {
                    eprintln!("Ghostlog: start watching failed: {e}");
                }
            }
            "stop-watching" => crate::watcher::stop(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .build(app)?;

    Ok(())
}

/// Reflect the current watch state on the tray icon (gray/red) + tooltip.
pub fn sync(app: &AppHandle) {
    let state = app.state::<AppState>();
    let watching = state.watch_state.lock().unwrap().state == WatchState::Watching;

    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let bytes = if watching { ICON_WATCHING } else { ICON_IDLE };
        if let Ok(icon) = tauri::image::Image::from_bytes(bytes) {
            let _ = tray.set_icon(Some(icon));
        }
        let _ = tray.set_tooltip(Some(if watching { "Ghostlog — watching" } else { "Ghostlog — idle" }));
    }
}

pub fn show_review_window(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.show();
        let _ = win.set_focus();
    }
}

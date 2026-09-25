//! The icon next to the Windows clock and its menu.

use crate::state::{Shared, Trigger};
use std::sync::Arc;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};

pub fn show_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// The status line for the tray menu and tooltip.
pub fn status_text(shared: &Shared) -> String {
    let live = shared.live();
    let pending = shared.queue().ok().and_then(|q| q.status().ok()).map(|s| s.pending).unwrap_or(0);
    if !shared.connected() {
        "Not connected to RestedRealm".into()
    } else if live.last_error.is_some() {
        "Needs attention".into()
    } else if pending > 0 {
        format!("{pending} waiting to upload")
    } else {
        "Up to date".into()
    }
}

pub fn refresh(shared: &Shared) {
    let text = status_text(shared);
    if let Some(item) = shared.tray_status.lock().unwrap().as_ref() {
        let _ = item.set_text(&text);
    }
}

pub fn build(app: &AppHandle, shared: Arc<Shared>) -> tauri::Result<()> {
    let status = MenuItem::with_id(app, "status", status_text(&shared), false, None::<&str>)?;
    let open = MenuItem::with_id(app, "open", "Open RestedRealm Companion", true, None::<&str>)?;
    let sync = MenuItem::with_id(app, "sync", "Upload now", true, None::<&str>)?;
    let site = MenuItem::with_id(app, "site", "Open restedrealm.com", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let separator2 = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&status, &separator, &open, &sync, &site, &separator2, &quit])?;
    *shared.tray_status.lock().unwrap() = Some(status);

    let for_menu = shared.clone();
    TrayIconBuilder::with_id("main")
        .icon(app.default_window_icon().cloned().expect("the bundle has an icon"))
        .tooltip("RestedRealm Companion")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(move |app, event| match event.id.as_ref() {
            "open" => show_window(app),
            "sync" => for_menu.nudge(Trigger::SyncNow),
            "site" => {
                use tauri_plugin_opener::OpenerExt;
                let _ = app.opener().open_url("https://restedrealm.com/", None::<&str>);
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                show_window(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

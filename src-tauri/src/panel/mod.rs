//! The bottom panel window: placement on the active display, show and hide.
//!
//! `show_panel` reads the display under the pointer through AppKit, so it must
//! run on the main thread. The tray menu handler already does.

pub mod geometry;

use tauri::{AppHandle, LogicalPosition, LogicalSize, Manager};

use crate::platform;
use geometry::{panel_frame, PANEL_HEIGHT};

/// Label of the one window defined in tauri.conf.json.
pub const WINDOW_LABEL: &str = "main";
/// Width used when display information is unavailable (non macOS builds).
const FALLBACK_WIDTH: f64 = 1512.0;

pub fn show_panel(app: &AppHandle) -> tauri::Result<()> {
    let Some(window) = app.get_webview_window(WINDOW_LABEL) else {
        return Ok(());
    };
    match platform::active_screen() {
        Some(screen) => {
            let f = panel_frame(screen.visible, screen.main_height);
            window.set_size(LogicalSize::new(f.width, f.height))?;
            window.set_position(LogicalPosition::new(f.x, f.y))?;
        }
        None => window.set_size(LogicalSize::new(FALLBACK_WIDTH, PANEL_HEIGHT))?,
    }
    window.show()?;
    window.set_focus()
}

pub fn hide_panel(app: &AppHandle) -> tauri::Result<()> {
    match app.get_webview_window(WINDOW_LABEL) {
        Some(window) => window.hide(),
        None => Ok(()),
    }
}

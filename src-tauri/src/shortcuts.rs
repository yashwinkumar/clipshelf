//! Global panel shortcut. Configuration lives alongside the history database.
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

const DEFAULT: &str = "Control+Alt+Space";

#[derive(Deserialize, Serialize)]
struct Settings {
    panel_shortcut: String,
}

pub fn install(app: &AppHandle) -> tauri::Result<()> {
    app.plugin(
        tauri_plugin_global_shortcut::Builder::new()
            .with_handler(|app, _, event| {
                if event.state() != ShortcutState::Pressed {
                    return;
                }
                let handle = app.clone();
                let _ = app.run_on_main_thread(move || {
                    if let Some(window) = handle.get_webview_window(crate::panel::WINDOW_LABEL) {
                        if window.is_visible().unwrap_or(false) {
                            let _ = crate::panel::hide_panel(&handle);
                        } else {
                            let _ = crate::panel::show_panel(&handle);
                        }
                    }
                });
            })
            .build(),
    )?;
    let path = crate::storage::paths::history_db_path(app)?.with_file_name("settings.json");
    let configured = match std::fs::read(&path) {
        Ok(bytes) => match serde_json::from_slice::<Settings>(&bytes) {
            Ok(settings) => settings.panel_shortcut,
            Err(_) => {
                eprintln!("[shortcut] settings are invalid; using default");
                DEFAULT.to_string()
            }
        },
        Err(_) => DEFAULT.to_string(),
    };
    if !path.exists() {
        let settings = Settings {
            panel_shortcut: DEFAULT.to_string(),
        };
        if let Ok(bytes) = serde_json::to_vec_pretty(&settings) {
            if std::fs::write(&path, bytes).is_err() {
                eprintln!("[shortcut] settings could not be saved");
            }
        }
    }
    match app.global_shortcut().register(configured.as_str()) {
        Ok(()) => eprintln!("[shortcut] panel shortcut registered"),
        Err(_) => {
            eprintln!("[shortcut] configured shortcut unavailable; trying default");
            if configured == DEFAULT || app.global_shortcut().register(DEFAULT).is_err() {
                eprintln!("[shortcut] default unavailable; use the tray Show panel menu");
            }
        }
    }
    Ok(())
}

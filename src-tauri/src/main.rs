// Clipshelf. Application setup only; behaviour lives in the modules below.

mod capture;
mod commands;
mod panel;
mod platform;
mod shortcuts;
mod storage;
mod tray;

use std::sync::Mutex;

use tauri::Manager;

use storage::{paths, HistoryState, Store};

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            let handle = app.clone();
            let _ = app.run_on_main_thread(move || {
                let _ = panel::show_panel(&handle);
            });
        }))
        .invoke_handler(tauri::generate_handler![
            commands::capture_status,
            commands::clear_history,
            commands::capture_paused,
            commands::set_capture_paused,
            commands::list_history,
            commands::search_history,
            commands::copy_history_entry,
            commands::hide_panel,
            commands::app_icon,
        ])
        .setup(|app| {
            let path = paths::history_db_path(app.handle())?;
            let store = Store::open(&path).map_err(|err| {
                eprintln!(
                    "[storage] could not open history database at {}: {err}",
                    path.display()
                );
                err
            })?;
            eprintln!("[storage] history database: {}", path.display());
            app.manage(HistoryState(Mutex::new(store)));

            capture::start(app.handle().clone());
            tray::build(app)?;
            shortcuts::install(app.handle())?;
            panel::show_panel(app.handle())?;
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building Clipshelf")
        .run(|app, event| {
            if let tauri::RunEvent::Reopen { .. } = event {
                let _ = panel::show_panel(app);
            }
        });
}

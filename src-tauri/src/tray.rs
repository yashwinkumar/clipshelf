//! Menu bar item: show the panel, quit.

use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    App,
};

pub fn build(app: &App) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "Show panel", true, None::<&str>)?;
    let pause = MenuItem::with_id(app, "pause", "Pause or resume capture", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &pause, &quit])?;

    let mut tray = TrayIconBuilder::new()
        .menu(&menu)
        .tooltip("Clipshelf (dev)")
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => {
                let _ = crate::panel::show_panel(app);
            }
            "pause" => {
                let _ = crate::capture::set_paused(app, !crate::capture::is_paused(app));
            }
            "quit" => app.exit(0),
            _ => {}
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(())
}

//! Where the history database lives. Development and release data never share a file.
//!
//! Base folder (macOS): `~/Library/Application Support/dev.promptlyashwin.clipshelf/`
//! - Debug builds (`npm run dev`):  `<base>/dev/history.sqlite3`
//! - Release builds:                `<base>/history.sqlite3`

use std::path::PathBuf;

use tauri::{AppHandle, Manager};

pub const FILE_NAME: &str = "history.sqlite3";
pub const DEV_FOLDER: &str = "dev";

pub fn history_db_path(app: &AppHandle) -> tauri::Result<PathBuf> {
    let base = app.path().app_data_dir()?;
    Ok(db_path_in(base, cfg!(debug_assertions)))
}

/// Pure path rule, separated so it can be tested without a running app.
pub fn db_path_in(base: PathBuf, development: bool) -> PathBuf {
    if development {
        base.join(DEV_FOLDER).join(FILE_NAME)
    } else {
        base.join(FILE_NAME)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn development_and_release_paths_differ() {
        let base = PathBuf::from("/x/app");
        let dev = db_path_in(base.clone(), true);
        let rel = db_path_in(base, false);
        assert_eq!(dev, PathBuf::from("/x/app/dev/history.sqlite3"));
        assert_eq!(rel, PathBuf::from("/x/app/history.sqlite3"));
        assert_ne!(dev, rel);
    }
}

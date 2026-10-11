//! Tauri commands exposed to the frontend. Thin: validation and delegation only.

use std::sync::mpsc;
use std::time::Duration;

use base64::Engine;
use tauri::{AppHandle, Emitter, Manager, State};

use serde::Serialize;

use crate::capture::state::{CaptureState, Counters};
use crate::capture::PollGuard;
use crate::platform;
use crate::storage::history::{Entry, DEFAULT_LIMIT};
use crate::storage::HistoryState;

/// Counters only. Clipboard text is returned only by the history commands below.
#[tauri::command]
pub fn capture_status(state: State<'_, CaptureState>) -> Counters {
    state.counters.lock().map(|c| *c).unwrap_or_default()
}

/// Newest entries first. `limit` is clamped by the storage layer.
#[tauri::command]
pub fn list_history(
    history: State<'_, HistoryState>,
    limit: Option<i64>,
) -> Result<Vec<Entry>, String> {
    let store = history
        .0
        .lock()
        .map_err(|_| "history is unavailable".to_string())?;
    store
        .list(limit.unwrap_or(DEFAULT_LIMIT))
        .map_err(|err| err.to_string())
}

/// Literal, case insensitive substring search. The query is never treated as SQL or as a pattern.
#[tauri::command]
pub fn search_history(
    history: State<'_, HistoryState>,
    query: String,
    limit: Option<i64>,
) -> Result<Vec<Entry>, String> {
    let store = history
        .0
        .lock()
        .map_err(|_| "history is unavailable".to_string())?;
    store
        .search(&query, limit.unwrap_or(DEFAULT_LIMIT))
        .map_err(|err| err.to_string())
}

#[derive(Serialize)]
pub struct CopyResult {
    pub chars: usize,
    /// True when the write was recognised as ours and will not be captured again.
    pub acknowledged: bool,
}

/// Put a stored entry back on the clipboard as plain text. The user pastes with
/// Command V; no key is simulated. Errors are fixed text with no content.
#[tauri::command]
pub fn copy_history_entry(
    history: State<'_, HistoryState>,
    poll: State<'_, PollGuard>,
    id: i64,
) -> Result<CopyResult, String> {
    let entry = {
        let store = history
            .0
            .lock()
            .map_err(|_| "history is unavailable".to_string())?;
        store.get(id).map_err(|err| err.to_string())?
    }
    .ok_or_else(|| "that entry no longer exists".to_string())?;
    let chars = entry.text.chars().count();

    // Hold the poll lock across the write and the acknowledgement.
    let mut debouncer = poll
        .0
        .lock()
        .map_err(|_| "capture is unavailable".to_string())?;
    let receipt = platform::write_text(&entry.text)
        .ok_or_else(|| "the clipboard could not be written".to_string())?;
    let acknowledged = debouncer.acknowledge_write(receipt.produced, receipt.current);
    eprintln!("[copy] id={id} chars={chars} acknowledged={acknowledged}");
    Ok(CopyResult {
        chars,
        acknowledged,
    })
}

/// Hide the panel window. Clipboard capture keeps running.
#[tauri::command]
pub fn hide_panel(app: AppHandle) -> Result<(), String> {
    crate::panel::hide_panel(&app).map_err(|_| "the panel could not be hidden".to_string())
}

/// Icon of an installed app as a PNG data URL, or `None` when unavailable.
/// AppKit work runs on the main thread; this command only waits for the answer.
#[tauri::command]
pub async fn app_icon(app: AppHandle, bundle_id: String) -> Result<Option<String>, String> {
    let (tx, rx) = mpsc::channel();
    app.run_on_main_thread(move || {
        let _ = tx.send(platform::app_icon_png(&bundle_id));
    })
    .map_err(|_| "icons are unavailable".to_string())?;
    let png = rx.recv_timeout(Duration::from_secs(2)).ok().flatten();
    Ok(png.map(|bytes| {
        format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(bytes)
        )
    }))
}

#[tauri::command]
pub fn capture_paused(app: AppHandle) -> bool {
    crate::capture::is_paused(&app)
}

#[tauri::command]
pub fn set_capture_paused(app: AppHandle, paused: bool) -> Result<(), String> {
    crate::capture::set_paused(&app, paused)
}

/// Delete all history only after explicit confirmation in the panel.
#[tauri::command]
pub fn clear_history(app: AppHandle, confirmed: bool) -> Result<usize, String> {
    if !confirmed {
        return Err("confirm clearing all history first".to_string());
    }
    let poll = app.state::<PollGuard>();
    let mut detector = poll
        .0
        .lock()
        .map_err(|_| "capture is unavailable".to_string())?;
    let history = app.state::<HistoryState>();
    let removed = history
        .0
        .lock()
        .map_err(|_| "history is unavailable".to_string())?
        .clear()
        .map_err(|_| "history could not be cleared".to_string())?;
    let capture = app.state::<CaptureState>();
    capture
        .generation
        .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    if let Ok(mut latest) = capture.latest.lock() {
        *latest = None;
    }
    let current = platform::change_count();
    detector.acknowledge_write(current, current);
    let _ = app.emit("history-cleared", ());
    Ok(removed)
}

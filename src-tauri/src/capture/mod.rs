//! Clipboard capture: poll, debounce, filter, read, store.
//!
//! Flow per change: debounce fires, read type names, apply the type filter,
//! and only then read text. Accepted text is written to the history database.
//! Logs and events carry metadata only, never content.

pub mod debounce;
pub mod filter;
pub mod state;

use std::sync::{atomic::Ordering, Mutex};
use std::thread;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::platform::{self, SourceApp};
use crate::storage::history::{InsertOutcome, Source};
use crate::storage::HistoryState;
use debounce::{Debouncer, DEBOUNCE, POLL_INTERVAL};
use filter::SkipReason;
use state::{CaptureState, CapturedText};

/// Event sent to the frontend as `clipboard-event`. No clipboard content.
#[derive(Clone, Serialize)]
pub struct CaptureEvent {
    pub change_count: i64,
    pub at_ms: u128,
    pub source_app: Option<SourceApp>,
    /// `captured` or `skipped`.
    pub outcome: &'static str,
    pub skip_reason: Option<SkipReason>,
    /// Which excluded type matched, when the reason is `excluded_type`.
    pub excluded_type: Option<&'static str>,
    pub type_count: usize,
    pub char_count: Option<usize>,
    pub byte_len: Option<usize>,
    /// Raw access behaviour of the pasteboard (macOS 15.4 and later).
    pub access_behavior: Option<i64>,
    /// History write result for captured text: `stored`, `duplicate`,
    /// `storage_error` or `unavailable`. `None` when nothing was captured.
    pub stored: Option<&'static str>,
}

/// The poller's change detector, shared so a copy back can acknowledge its own
/// write. Both the poll sample and the copy back hold this lock for their whole
/// step, so the poller never samples between our write and its acknowledgement.
pub struct PollGuard(pub Mutex<Debouncer>);

/// Start the polling thread.
pub fn start(app: AppHandle) {
    app.manage(CaptureState::default());
    app.manage(PollGuard(Mutex::new(Debouncer::new(
        platform::change_count(),
        DEBOUNCE,
    ))));
    thread::Builder::new()
        .name("clipboard-poller".into())
        .spawn(move || {
            eprintln!(
                "[capture] started, interval {:?}, debounce {:?}",
                POLL_INTERVAL, DEBOUNCE
            );
            loop {
                thread::sleep(POLL_INTERVAL);
                let fired = {
                    let guard = app.state::<PollGuard>();
                    let Ok(mut debouncer) = guard.0.lock() else {
                        continue;
                    };
                    let current = platform::change_count();
                    if app.state::<CaptureState>().paused.load(Ordering::SeqCst) {
                        debouncer.acknowledge_write(current, current);
                        None
                    } else {
                        debouncer.observe(Instant::now(), current).map(|fired| {
                            (
                                fired,
                                app.state::<CaptureState>()
                                    .generation
                                    .load(Ordering::SeqCst),
                            )
                        })
                    }
                };
                if let Some((fired, generation)) = fired {
                    let handle = app.clone();
                    // AppKit state is read on the main thread.
                    let _ = app.run_on_main_thread(move || {
                        if let Some(event) = process_change(&handle, fired.change_count, generation)
                        {
                            log_event(&event);
                            let _ = handle.emit("clipboard-event", event);
                        }
                    });
                }
            }
        })
        .expect("failed to start clipboard poller thread");
}

fn process_change(app: &AppHandle, change_count: i64, generation: u64) -> Option<CaptureEvent> {
    // The clipboard moved on since this change was detected (another copy, or
    // our own copy back). The poller will report the newer state, so skip.
    if is_paused(app)
        || app
            .state::<CaptureState>()
            .generation
            .load(Ordering::SeqCst)
            != generation
        || platform::change_count() != change_count
    {
        return None;
    }
    let at_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let types = platform::read_types();
    let mut event = CaptureEvent {
        change_count,
        at_ms,
        source_app: platform::frontmost_app(),
        outcome: "skipped",
        skip_reason: None,
        excluded_type: None,
        type_count: types.len(),
        char_count: None,
        byte_len: None,
        access_behavior: platform::access_behavior(),
        stored: None,
    };

    // Step 1: type based exclusion. Nothing is read if this matches.
    if let Some(matched) = filter::excluded_type(&types) {
        event.skip_reason = Some(SkipReason::ExcludedType);
        event.excluded_type = Some(matched);
        count(app, false);
        return Some(event);
    }
    if !filter::has_text_type(&types) {
        event.skip_reason = Some(SkipReason::NoText);
        count(app, false);
        return Some(event);
    }

    // Step 2: read text, then apply text checks.
    match platform::read_text() {
        None => {
            // A text type was listed but no text came back.
            event.skip_reason = Some(SkipReason::ReadFailed);
            count(app, false);
        }
        Some(text) => {
            if let Some(reason) = filter::check_text(&text) {
                event.skip_reason = Some(reason);
                event.byte_len = Some(text.len());
                count(app, false);
            } else {
                let held = CapturedText::new(text);
                event.outcome = "captured";
                event.char_count = Some(held.char_count());
                event.byte_len = Some(held.as_str().len());
                event.stored = Some(store_entry(app, &held, at_ms, event.source_app.as_ref()));
                if let Some(state) = app.try_state::<CaptureState>() {
                    if let Ok(mut latest) = state.latest.lock() {
                        *latest = Some(held);
                    }
                }
                count(app, true);
            }
        }
    }
    Some(event)
}

/// Write accepted text to history. Returns the outcome label only, never the text.
fn store_entry(
    app: &AppHandle,
    text: &CapturedText,
    at_ms: u128,
    source: Option<&SourceApp>,
) -> &'static str {
    let Some(history) = app.try_state::<HistoryState>() else {
        return "unavailable";
    };
    let Ok(mut store) = history.0.lock() else {
        return "storage_error";
    };
    let meta = Source {
        bundle_id: source.and_then(|s| s.bundle_id.clone()),
        name: source.and_then(|s| s.name.clone()),
    };
    match store.insert(text.as_str(), at_ms as i64, &meta) {
        Ok(InsertOutcome::Stored(_)) => "stored",
        Ok(InsertOutcome::Duplicate) => "duplicate",
        Err(err) => {
            // Display text only. It is generic and carries no clipboard content.
            eprintln!("[storage] insert failed: {err}");
            "storage_error"
        }
    }
}

fn count(app: &AppHandle, captured: bool) {
    if let Some(state) = app.try_state::<CaptureState>() {
        if let Ok(mut c) = state.counters.lock() {
            if captured {
                c.captured += 1;
            } else {
                c.skipped += 1;
            }
        }
    }
}

/// Metadata only. The event type has no content field, so it cannot leak.
fn log_event(event: &CaptureEvent) {
    eprintln!(
        "[capture] change={} outcome={} reason={} excluded={} types={} chars={} stored={} source={} access={}",
        event.change_count,
        event.outcome,
        event.skip_reason.map(|r| format!("{:?}", r)).unwrap_or_else(|| "-".into()),
        event.excluded_type.unwrap_or("-"),
        event.type_count,
        event.char_count.map(|n| n.to_string()).unwrap_or_else(|| "-".into()),
        event.stored.unwrap_or("-"),
        event
            .source_app
            .as_ref()
            .and_then(|s| s.bundle_id.clone())
            .unwrap_or_else(|| "unknown".into()),
        event.access_behavior.map(|n| n.to_string()).unwrap_or_else(|| "-".into()),
    );
}

/// Capture state is shared by the tray and panel controls.
pub fn is_paused(app: &AppHandle) -> bool {
    app.state::<CaptureState>().paused.load(Ordering::SeqCst)
}

/// Rebase both transitions so paused copies are never imported on resume.
pub fn set_paused(app: &AppHandle, paused: bool) -> Result<(), String> {
    let guard = app.state::<PollGuard>();
    let mut detector = guard
        .0
        .lock()
        .map_err(|_| "capture is unavailable".to_string())?;
    let current = platform::change_count();
    app.state::<CaptureState>()
        .generation
        .fetch_add(1, Ordering::SeqCst);
    detector.acknowledge_write(current, current);
    app.state::<CaptureState>()
        .paused
        .store(paused, Ordering::SeqCst);
    let _ = app.emit("capture-paused", paused);
    Ok(())
}

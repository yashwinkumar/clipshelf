//! In memory capture state. M2 keeps only the latest text; persistence is M3.

use std::sync::{
    atomic::{AtomicBool, AtomicU64},
    Mutex,
};

/// Text held in memory. `Debug` is redacted so it cannot reach a log by accident.
pub struct CapturedText(String);

impl CapturedText {
    pub fn new(text: String) -> Self {
        Self(text)
    }
    /// Text for storage and, in a later milestone, copy back.
    pub fn as_str(&self) -> &str {
        &self.0
    }
    pub fn char_count(&self) -> usize {
        self.0.chars().count()
    }
}

impl std::fmt::Debug for CapturedText {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CapturedText(<{} chars redacted>)", self.char_count())
    }
}

#[derive(Default)]
pub struct CaptureState {
    pub paused: AtomicBool,
    pub generation: AtomicU64,
    pub latest: Mutex<Option<CapturedText>>,
    pub counters: Mutex<Counters>,
}

#[derive(Default, Clone, Copy, serde::Serialize)]
pub struct Counters {
    pub captured: u64,
    pub skipped: u64,
}

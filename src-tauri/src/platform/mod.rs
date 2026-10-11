//! Platform access to the system clipboard and the frontmost application.
//!
//! Only this module talks to operating system APIs. Everything else works on
//! plain Rust values, so it can be tested without a window or a pasteboard.

use serde::Serialize;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::*;

#[cfg(not(target_os = "macos"))]
mod unsupported;
#[cfg(not(target_os = "macos"))]
pub use unsupported::*;

/// The application that was in front when the clipboard change was read.
#[derive(Clone, Debug, Serialize)]
pub struct SourceApp {
    pub bundle_id: Option<String>,
    pub name: Option<String>,
}

/// Change counts around a clipboard write made by this app.
#[derive(Clone, Copy, Debug)]
pub struct WriteReceipt {
    /// Count created by our write.
    pub produced: i64,
    /// Count read right after the write. Differs if another app wrote meanwhile.
    pub current: i64,
}

/// A rectangle in points, AppKit coordinates (origin bottom left of the main display).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScreenRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// The display under the pointer: its visible area (menu bar and Dock excluded),
/// and the height of the main display for the top-left coordinate conversion.
#[derive(Clone, Copy, Debug)]
pub struct ActiveScreen {
    pub visible: ScreenRect,
    pub main_height: f64,
}
